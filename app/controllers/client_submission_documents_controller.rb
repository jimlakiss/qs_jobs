class ClientSubmissionDocumentsController < ApplicationController
  before_action :set_client_submission
  before_action :authorize_client_submission!
  before_action :ensure_draft!
  before_action :set_document, only: [:update, :destroy]

  def create
    if document_params[:documents].present?
      existing_attachment_ids = @client_submission.documents.attachments.ids
      @client_submission.documents.attach(document_params[:documents])
      create_document_metadata(existing_attachment_ids, document_group: document_group_from_upload_params)
      redirect_to @client_submission, notice: "Documents uploaded"
    else
      redirect_to @client_submission, alert: "Choose at least one document to upload"
    end
  end

  def update
    @document.update!(client_submission_document_params.merge(document_group: document_group_from_metadata_params))
    redirect_to @client_submission, notice: "Document details updated"
  end

  def destroy
    attachment = @client_submission.documents.find(@document.active_storage_attachment_id)
    @document.destroy
    attachment.purge

    redirect_to @client_submission, notice: "Document removed"
  end

  private

  def set_client_submission
    @client_submission = ClientSubmission.find(params[:client_submission_id])
  end

  def authorize_client_submission!
    return if admin_user?
    return if @client_submission.submitted_by == current_user && current_user.client_upload_access?

    redirect_to client_submissions_path, alert: "You do not have access to that submission"
  end

  def ensure_draft!
    return if admin_user? || @client_submission.draft?

    redirect_to @client_submission, alert: "Submitted documents cannot be changed"
  end

  def set_document
    @document = @client_submission.client_submission_documents.find(params[:id])
  end

  def document_params
    params.fetch(:client_submission, {}).permit(:document_group_name, documents: [])
  end

  def client_submission_document_params
    params.require(:client_submission_document).permit(:display_name, :notes)
  end

  def create_document_metadata(existing_attachment_ids, document_group: nil)
    @client_submission.documents.attachments.where.not(id: existing_attachment_ids).find_each do |attachment|
      @client_submission.client_submission_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |document|
        document.display_name = attachment.filename.to_s
        document.document_group = document_group
      end
    end
  end

  def document_group_from_upload_params
    find_or_create_document_group(document_params[:document_group_name])
  end

  def document_group_from_metadata_params
    find_or_create_document_group(params.dig(:client_submission_document, :document_group_name))
  end

  def find_or_create_document_group(name)
    normalized_name = name.to_s.strip
    return nil if normalized_name.blank?

    @client_submission.document_groups.where("LOWER(name) = LOWER(?)", normalized_name).first_or_create!(name: normalized_name)
  end
end
