require "zip"

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
    @viewer_navigation_documents = []
    render "project_documents/viewer"
  end

  def update
    @document_group.update!(document_group_params)
    redirect_to project_path(@project, tab: document_tab_param), notice: "Folder renamed"
  end

  def download_working_documents
    documents = working_pdf_documents

    if documents.empty?
      redirect_to project_path(@project, tab: "working-documents"), alert: "This folder does not contain any working PDF documents"
      return
    end

    zip_stream = Zip::OutputStream.write_buffer do |zip|
      documents.each do |document|
        zip.put_next_entry(document.filename.to_s)
        zip.write(document.download)
      end
    end
    zip_stream.rewind

    send_data zip_stream.read,
      filename: "#{safe_zip_filename(@document_group.name)}.zip",
      type: "application/zip",
      disposition: "attachment"
  end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end

  def set_document_group
    @document_group = @project.document_groups.find(params[:id])
  end

  def document_group_params
    params.require(:document_group).permit(:name)
  end

  def document_tab_param
    params[:tab].presence_in(%w[imported-documents extracted-data measured-dimensions working-documents extracted-documents]) || "imported-documents"
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

  def working_pdf_documents
    attachment_ids = @project.project_documents
      .where(document_group: @document_group, category: "extracted_document")
      .pluck(:active_storage_attachment_id)

    @project.documents
      .attachments
      .includes(:blob)
      .where(id: attachment_ids)
      .select { |document| pdf_document?(document) }
      .sort_by { |document| document.filename.to_s.downcase }
  end

  def safe_zip_filename(name)
    name.to_s.gsub(%r{[/\\:*?"<>|]}, "").squish.presence || "working-documents"
  end

  def viewer_navigation_documents
    metadata_by_attachment_id = @project.project_documents.includes(:document_group).index_by(&:active_storage_attachment_id)

    @project.documents.attachments.includes(:blob).filter_map do |attachment|
      metadata = metadata_by_attachment_id[attachment.id]
      next unless metadata&.category == "extracted_document"
      next unless pdf_document?(attachment)

      {
        id: attachment.id,
        name: attachment.filename.to_s,
        group: metadata&.document_group&.name.presence || "Ungrouped",
        size: attachment.byte_size,
        receivedAt: (metadata&.received_at || attachment.created_at)&.iso8601,
        exportKind: metadata&.export_kind,
        url: rails_service_blob_proxy_path(attachment.blob.signed_id, attachment.filename),
        saveExtractionUrl: save_extraction_project_document_path(@project, attachment),
        viewerStateUrl: viewer_state_project_document_path(@project, attachment),
        uploadExportUrl: upload_export_project_document_path(@project, attachment)
      }
    end.sort_by { |item| [item[:group].downcase, item[:name].downcase] }
  end
end
