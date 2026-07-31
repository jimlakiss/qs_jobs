class ClientSubmissionsController < ApplicationController
  before_action :require_client_upload_access!, only: [:new, :create]
  before_action :set_client_submission, only: [:show, :edit, :update, :destroy, :submit, :convert, :attach_to_project, :archive, :unarchive]
  before_action :authorize_client_submission!, only: [:show, :edit, :update, :destroy, :submit]
  before_action :require_admin!, only: [:convert, :attach_to_project, :archive, :unarchive]

  def index
    @client_submissions =
      if admin_user?
        @show_archived = ActiveModel::Type::Boolean.new.cast(params[:archived])
        admin_client_submissions_scope.includes(:contributor, :submitted_by, :project).latest_first
      else
        @project_issues = current_user.received_project_issues.includes(:project, :contributor).available_to_clients
        current_user.client_submissions.includes(:project).latest_first
      end
  end

  def show
    sync_document_metadata!
    @document_groups = @client_submission.document_groups.alphabetical
    @documents_by_attachment_id = @client_submission.client_submission_documents.includes(:document_group).index_by(&:active_storage_attachment_id)
    @project = Project.new(address: @client_submission.address, description: @client_submission.description)
    @existing_projects = Project.order(:code) if admin_user? && processable_by_admin?
  end

  def new
    @client_submission = current_user.client_submissions.new(contributor: current_user.contributor)
  end

  def create
    @client_submission = current_user.client_submissions.new(client_submission_params)
    @client_submission.contributor = current_user.contributor

    if @client_submission.save
      attach_documents(document_group: document_group_from_upload_params)
      redirect_to @client_submission, notice: "Submission draft created"
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    if editable_by_client? && @client_submission.update(client_submission_params)
      attach_documents(document_group: document_group_from_upload_params)
      redirect_to @client_submission, notice: "Submission updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    if editable_by_client?
      @client_submission.destroy
      redirect_to client_submissions_path, notice: "Submission deleted"
    else
      redirect_to @client_submission, alert: "Submitted items cannot be deleted"
    end
  end

  def submit
    if editable_by_client?
      @client_submission.submit!
      ClientSubmissionMailer.project_uploaded(@client_submission, recipient: @client_submission.submitted_by.email).deliver_later
      ClientSubmissionMailer.project_uploaded(@client_submission, recipient: ClientSubmissionMailer.internal_recipients).deliver_later
      redirect_to @client_submission, notice: "Submission sent"
    else
      redirect_to @client_submission, alert: "This submission has already been sent"
    end
  rescue ActiveRecord::RecordInvalid
    show
    flash.now[:alert] = @client_submission.errors.full_messages.to_sentence
    render :show, status: :unprocessable_entity
  end

  def convert
    project = Project.new(convert_project_params)

    if project.save
      attach_submission_documents_to(project)
      @client_submission.update!(status: :converted, project: project)
      redirect_to project, notice: "Client submission converted to project"
    else
      show
      @project = project
      render :show, status: :unprocessable_entity
    end
  end

  def attach_to_project
    project = Project.find(attach_to_project_params[:project_id])

    attach_submission_documents_to(project)
    @client_submission.update!(status: :converted, project: project)

    redirect_to project, notice: "Client submission added to existing project"
  rescue ActiveRecord::RecordNotFound
    show
    flash.now[:alert] = "Choose an existing project"
    render :show, status: :unprocessable_entity
  end

  def archive
    @client_submission.update!(archived_at: Time.current)
    redirect_to client_submissions_path, notice: "Client submission archived"
  end

  def unarchive
    @client_submission.update!(archived_at: nil)
    redirect_to client_submissions_path(archived: true), notice: "Client submission restored"
  end

  private

  def set_client_submission
    @client_submission = ClientSubmission.find(params[:id])
  end

  def authorize_client_submission!
    return if admin_user?
    return if @client_submission.submitted_by == current_user

    redirect_to client_submissions_path, alert: "You do not have access to that submission"
  end

  def require_client_upload_access!
    return if current_user.client_upload_access?

    redirect_to client_submissions_path, alert: "Project upload access has not been enabled for this login"
  end

  def admin_client_submissions_scope
    @show_archived ? ClientSubmission.archived : ClientSubmission.active
  end

  def editable_by_client?
    admin_user? || @client_submission.draft?
  end

  def processable_by_admin?
    admin_user? && !@client_submission.converted?
  end

  def client_submission_params
    params.require(:client_submission).permit(
      :client_reference_code,
      :address,
      :description,
      :required_by,
      :notes,
      documents: []
    )
  end

  def convert_project_params
    params.require(:project).permit(:code, :date, :address, :description)
  end

  def attach_to_project_params
    params.require(:client_submission).permit(:project_id)
  end

  def attach_documents(document_group: nil)
    return if client_submission_params[:documents].blank?

    existing_attachment_ids = @client_submission.documents.attachments.ids
    @client_submission.documents.attach(client_submission_params[:documents])
    create_document_metadata(existing_attachment_ids, document_group: document_group)
  end

  def create_document_metadata(existing_attachment_ids, document_group: nil)
    @client_submission.documents.attachments.where.not(id: existing_attachment_ids).find_each do |attachment|
      @client_submission.client_submission_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |document|
        document.display_name = attachment.filename.to_s
        document.document_group = document_group
      end
    end
  end

  def sync_document_metadata!
    create_document_metadata([])
  end

  def attach_submission_documents_to(project)
    existing_attachment_ids = project.documents.attachments.ids
    project.documents.attach(@client_submission.documents.map(&:blob))
    project.project_contributors.find_or_create_by!(role: "Client") do |project_contributor|
      project_contributor.contributor = @client_submission.contributor
    end

    project.documents.attachments.where.not(id: existing_attachment_ids).each_with_index do |attachment, index|
      source_attachment = @client_submission.documents.attachments[index]
      source_metadata = @client_submission.client_submission_documents.find_by(active_storage_attachment_id: source_attachment&.id)
      document_group = project_document_group_for(project, source_metadata&.document_group)

      project.project_documents.create!(
        active_storage_attachment_id: attachment.id,
        category: "imported",
        document_group: document_group,
        received_at: @client_submission.submitted_at || @client_submission.updated_at,
        source: "Client portal",
        received_from: @client_submission.submitted_by.email,
        notes: source_metadata&.notes.presence || @client_submission.notes
      )
    end
  end

  def document_group_from_upload_params
    find_or_create_document_group(params.dig(:client_submission, :document_group_name))
  end

  def find_or_create_document_group(name)
    normalized_name = name.to_s.strip
    return nil if normalized_name.blank?

    @client_submission.document_groups.where("LOWER(name) = LOWER(?)", normalized_name).first_or_create!(name: normalized_name)
  end

  def project_document_group_for(project, source_group)
    return nil unless source_group

    project.document_groups.where("LOWER(name) = LOWER(?)", source_group.name).first_or_create!(name: source_group.name)
  end
end
