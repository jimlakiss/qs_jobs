class ClientSubmissionsController < ApplicationController
  before_action :require_client_upload_access!, only: [:new, :create]
  before_action :set_client_submission, only: [:show, :edit, :update, :destroy, :submit, :convert, :attach_to_project]
  before_action :authorize_client_submission!, only: [:show, :edit, :update, :destroy, :submit]
  before_action :require_admin!, only: [:convert, :attach_to_project]

  def index
    @client_submissions =
      if admin_user?
        ClientSubmission.includes(:contributor, :submitted_by, :project).latest_first
      else
        current_user.client_submissions.includes(:project).latest_first
      end
  end

  def show
    @documents_by_attachment_id = @client_submission.client_submission_documents.index_by(&:active_storage_attachment_id)
    @project = Project.new(address: @client_submission.address, description: @client_submission.description)
    @existing_projects = Project.order(:code) if admin_user? && @client_submission.submitted?
  end

  def new
    @client_submission = current_user.client_submissions.new(contributor: current_user.contributor)
  end

  def create
    @client_submission = current_user.client_submissions.new(client_submission_params)
    @client_submission.contributor = current_user.contributor

    if @client_submission.save
      attach_documents
      redirect_to @client_submission, notice: "Submission draft created"
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    if editable_by_client? && @client_submission.update(client_submission_params)
      attach_documents
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

  def editable_by_client?
    admin_user? || @client_submission.draft?
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

  def attach_documents
    return if client_submission_params[:documents].blank?

    existing_attachment_ids = @client_submission.documents.attachments.ids
    @client_submission.documents.attach(client_submission_params[:documents])
    create_document_metadata(existing_attachment_ids)
  end

  def create_document_metadata(existing_attachment_ids)
    @client_submission.documents.attachments.where.not(id: existing_attachment_ids).find_each do |attachment|
      @client_submission.client_submission_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |document|
        document.display_name = attachment.filename.to_s
      end
    end
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

      project.project_documents.create!(
        active_storage_attachment_id: attachment.id,
        category: "imported",
        received_at: @client_submission.submitted_at || @client_submission.updated_at,
        source: "Client portal",
        received_from: @client_submission.submitted_by.email,
        notes: source_metadata&.notes.presence || @client_submission.notes
      )
    end
  end
end
