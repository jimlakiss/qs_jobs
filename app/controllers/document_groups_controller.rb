class DocumentGroupsController < ApplicationController
  layout false, only: :viewer

  before_action :require_admin!
  before_action :set_project
  before_action :set_document_group

  def viewer
    @viewer_documents = grouped_pdf_documents

    if @viewer_documents.empty?
      redirect_to @project, alert: "This document group does not contain any PDF files"
      return
    end

    @document = @viewer_documents.first
    @viewer_title = @document_group.name
    render "project_documents/viewer"
  end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end

  def set_document_group
    @document_group = @project.document_groups.find(params[:id])
  end

  def grouped_pdf_documents
    attachment_ids = @project.project_documents
      .where(document_group: @document_group, category: "imported")
      .pluck(:active_storage_attachment_id)

    @project.documents
      .attachments
      .includes(:blob)
      .where(id: attachment_ids)
      .select { |document| pdf_document?(document) }
      .sort_by { |document| document.filename.to_s.downcase }
  end

  def pdf_document?(document)
    document.content_type == "application/pdf" || document.filename.extension.to_s.casecmp("pdf").zero?
  end
end
