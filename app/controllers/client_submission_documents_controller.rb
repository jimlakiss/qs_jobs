class ClientSubmissionDocumentsController < ApplicationController
  before_action :set_client_submission
  before_action :authorize_client_submission!
  before_action :ensure_draft!
  before_action :set_document, only: [:update, :destroy]

  def create
    if document_params[:documents].present?
      existing_attachment_ids = @client_submission.documents.attachments.ids
      @client_submission.documents.attach(document_params[:documents])
      create_document_metadata(existing_attachment_ids)
      redirect_to @client_submission, notice: "Documents uploaded"
    else
      redirect_to @client_submission, alert: "Choose at least one document to upload"
    end
  end

  def update
    @document.update!(client_submission_document_params)
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
    redirect_to @client_submission, alert: "Submitted documents cannot be changed" unless @client_submission.draft?
  end

  def set_document
    @document = @client_submission.client_submission_documents.find(params[:id])
  end

  def document_params
    params.fetch(:client_submission, {}).permit(documents: [])
  end

  def client_submission_document_params
    params.require(:client_submission_document).permit(:display_name, :notes)
  end

  def create_document_metadata(existing_attachment_ids)
    @client_submission.documents.attachments.where.not(id: existing_attachment_ids).find_each do |attachment|
      @client_submission.client_submission_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |document|
        document.display_name = attachment.filename.to_s
      end
    end
  end
end
