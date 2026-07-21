class ProjectDocumentsController < ApplicationController
  layout false, only: :viewer

  before_action :set_project
  before_action :set_document, only: [:viewer, :update, :destroy, :save_extraction, :upload_export]

  def viewer
    redirect_to @project, alert: "Only PDF documents can be opened in the viewer" unless pdf_document?
  end

  def create
    if document_params[:documents].present?
      existing_attachment_ids = @project.documents.attachments.ids
      @project.documents.attach(document_params[:documents])
      create_import_metadata(existing_attachment_ids)
      redirect_to @project, notice: "Documents uploaded"
    else
      redirect_to @project, alert: "Choose at least one document to upload"
    end
  end

  def update
    metadata = @project.project_documents.find_or_initialize_by(active_storage_attachment_id: @document.id)
    metadata.category ||= "imported"
    metadata.update!(document_metadata_params)

    redirect_to project_path(@project, tab: document_tab_param), notice: "Document details updated"
  end

  def destroy
    @project.project_documents.find_by(active_storage_attachment_id: @document.id)&.destroy
    @document.purge

    redirect_to @project, notice: "Document removed"
  end

  def save_extraction
    extraction = @project.document_extractions.find_or_initialize_by(active_storage_attachment_id: @document.id)
    payload = extraction_payload

    extraction.assign_attributes(
      document_details: payload[:document_details],
      sheets: payload[:sheets],
      regions: payload[:regions],
      measurements: payload[:measurements],
      staging_data: payload[:staging_data],
      source_filename: @document.filename.to_s,
      extracted_at: Time.current
    )
    extraction.save!

    render json: { ok: true, extracted_at: extraction.extracted_at.iso8601 }
  end

  def upload_export
    file = params.require(:file)
    existing_attachment_ids = @project.documents.attachments.ids
    @project.documents.attach(file)
    create_export_metadata(existing_attachment_ids, file)

    render json: { ok: true, filename: file.original_filename }
  end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end

  def set_document
    @document = @project.documents.find(params[:id])
  end

  def document_params
    params.fetch(:project, {}).permit(:received_at, :source, :received_from, :notes, documents: [])
  end

  def document_metadata_params
    params.fetch(:project_document, {}).permit(:received_at, :source, :received_from, :notes)
  end

  def document_tab_param
    params[:tab].presence_in(%w[imported-documents extracted-documents]) || "imported-documents"
  end

  def pdf_document?
    @document.content_type == "application/pdf" || @document.filename.extension.to_s.casecmp("pdf").zero?
  end

  def extraction_payload
    extraction = params.require(:extraction)

    {
      document_details: normalize_json(extraction[:document_details], {}),
      sheets: normalized_sheet_rows(extraction),
      regions: normalize_json(extraction[:regions], {}),
      measurements: normalize_json(extraction[:measurements], {}),
      staging_data: normalize_json(extraction[:staging_data], [])
    }
  end

  def normalized_sheet_rows(extraction)
    staging_data = normalize_json(extraction[:staging_data], [])
    staged_sheets = staging_data.filter_map.with_index do |sheet, index|
      next unless sheet.is_a?(Hash)

      {
        order: sheet["order"] || index + 1,
        page: sheet["page"],
        filename: sheet["filename"],
        sheet_id: sheet["sheet_id"],
        description: sheet["description"],
        issue_id: sheet["issue_id"],
        date: sheet["date"],
        issue_description: sheet["issue_description"],
        status: sheet["status"],
        included: sheet.fetch("included", true)
      }.compact
    end

    staged_sheets.presence || normalize_json(extraction[:sheets], [])
  end

  def normalize_json(value, fallback)
    return fallback if value.blank?

    JSON.parse(value.to_json)
  end

  def new_attachments(existing_attachment_ids)
    @project.documents.attachments.where.not(id: existing_attachment_ids)
  end

  def create_import_metadata(existing_attachment_ids)
    new_attachments(existing_attachment_ids).find_each do |attachment|
      @project.project_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |project_document|
        project_document.category = "imported"
        project_document.received_at = document_params[:received_at].presence || Time.current
        project_document.source = document_params[:source]
        project_document.received_from = document_params[:received_from]
        project_document.notes = document_params[:notes]
      end
    end
  end

  def create_export_metadata(existing_attachment_ids, file)
    new_attachments(existing_attachment_ids).find_each do |attachment|
      @project.project_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |project_document|
        project_document.category = "extracted_document"
        project_document.received_at = Time.current
        project_document.source = "PDF viewer"
        project_document.received_from = "QS Jobs"
        project_document.export_kind = params[:kind]
        project_document.generated_from_attachment_id = @document.id
        project_document.document_extraction = current_extraction
        project_document.notes = "Generated from #{@document.filename} via PDF viewer"
      end
    end
  end

  def current_extraction
    @project.document_extractions.where(active_storage_attachment_id: @document.id).order(extracted_at: :desc, updated_at: :desc).first
  end
end
