class ProjectDocumentsController < ApplicationController
  layout false, only: :viewer

  before_action :require_admin!
  before_action :set_project
  before_action :set_document, only: [:viewer, :update, :destroy, :save_extraction, :viewer_state, :upload_export]

  def viewer
    redirect_to @project, alert: "Only PDF documents can be opened in the viewer" unless pdf_document?
    @viewer_state = current_viewer_state&.data || {}
    @viewer_document_metadata = @project.project_documents.find_by(active_storage_attachment_id: @document.id)
    @viewer_navigation_documents = working_document_session? ? viewer_navigation_documents : []
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
    existing_attachment_ids = project_document_attachment_ids
    @project.documents.attach(file)
    new_attachment = newly_attached_document(existing_attachment_ids)
    create_export_metadata(new_attachment) if new_attachment

    render json: { ok: true, filename: file.original_filename, attachment_id: new_attachment&.id }
  end

  def viewer_state
    if request.get?
      render json: { viewer_state: current_viewer_state&.data || {} }
      return
    end

    state = current_viewer_state || @project.document_viewer_states.build(active_storage_attachment_id: @document.id)
    state.assign_attributes(
      data: normalize_json(params[:viewer_state], {}),
      saved_at: Time.current
    )
    state.save!

    render json: { ok: true, saved_at: state.saved_at.iso8601 }
  end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end

  def set_document
    @document = @project.documents.find(params[:id])
  end

  def document_params
    params.fetch(:project, {}).permit(:received_at, :source, :received_from, :notes, :document_group_name, documents: [])
  end

  def document_metadata_params
    metadata_category = @project.project_documents.find_by(active_storage_attachment_id: @document.id)&.category
    params.fetch(:project_document, {}).permit(:received_at, :source, :received_from, :notes)
      .merge(document_group: document_group_from_metadata_params(metadata_category))
  end

  def document_tab_param
    params[:tab].presence_in(%w[imported-documents extracted-data measured-dimensions working-documents extracted-documents]) || "imported-documents"
  end

  def pdf_document?
    @document.content_type == "application/pdf" || @document.filename.extension.to_s.casecmp("pdf").zero?
  end

  def pdf_attachment?(attachment)
    attachment.content_type == "application/pdf" || attachment.filename.extension.to_s.casecmp("pdf").zero?
  end

  def viewer_navigation_documents
    metadata_by_attachment_id = @project.project_documents.includes(:document_group).index_by(&:active_storage_attachment_id)

    @project.documents.attachments.includes(:blob).filter_map do |attachment|
      metadata = metadata_by_attachment_id[attachment.id]
      next unless metadata&.category == "extracted_document"
      next unless pdf_attachment?(attachment)

      {
        id: attachment.id,
        name: attachment.filename.to_s,
        group: metadata&.document_group&.name.presence || "Ungrouped",
        size: attachment.byte_size,
        receivedAt: (metadata&.received_at || attachment.created_at)&.iso8601,
        exportKind: metadata&.export_kind,
        url: rails_service_blob_path(attachment.blob.signed_id, attachment.filename),
        saveExtractionUrl: save_extraction_project_document_path(@project, attachment),
        viewerStateUrl: viewer_state_project_document_path(@project, attachment),
        uploadExportUrl: upload_export_project_document_path(@project, attachment)
      }
    end.sort_by { |item| [item[:group].downcase, item[:name].downcase] }
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
    document_group = document_group_from_upload_params

    new_attachments(existing_attachment_ids).find_each do |attachment|
      @project.project_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |project_document|
        project_document.category = "imported"
        project_document.document_group = document_group
        project_document.received_at = document_params[:received_at].presence || Time.current
        project_document.source = document_params[:source]
        project_document.received_from = document_params[:received_from]
        project_document.notes = document_params[:notes]
      end
    end
  end

  def create_export_metadata(attachment)
    @project.project_documents.find_or_create_by!(active_storage_attachment_id: attachment.id) do |project_document|
      project_document.category = "extracted_document"
      project_document.received_at = Time.current
      project_document.source = "PDF viewer"
      project_document.received_from = "iQs Jobs"
      project_document.export_kind = params[:kind]
      project_document.generated_from_attachment_id = @document.id
      project_document.document_extraction = current_extraction
      project_document.document_group = export_document_group || working_group_from_source_group
      project_document.notes = "Generated from #{@document.filename} via PDF viewer"
    end
  end

  def project_document_attachment_ids
    ActiveStorage::Attachment.where(record: @project, name: "documents").pluck(:id)
  end

  def newly_attached_document(existing_attachment_ids)
    ActiveStorage::Attachment
      .where(record: @project, name: "documents")
      .where.not(id: existing_attachment_ids)
      .order(:id)
      .last
  end

  def export_document_group
    find_or_create_document_group(params[:document_group_name], category: "working")
  end

  def working_group_from_source_group
    source_group_name = current_document_metadata&.document_group&.name
    find_or_create_document_group(source_group_name, category: "working")
  end

  def current_document_metadata
    @current_document_metadata ||= @project.project_documents.find_by(active_storage_attachment_id: @document.id)
  end

  def working_document_session?
    @viewer_document_metadata&.category == "extracted_document"
  end

  def current_extraction
    @project.document_extractions.where(active_storage_attachment_id: @document.id).order(extracted_at: :desc, updated_at: :desc).first
  end

  def current_viewer_state
    @current_viewer_state ||= @project.document_viewer_states.find_by(active_storage_attachment_id: @document.id)
  end

  def document_group_from_upload_params
    find_or_create_document_group(document_params[:document_group_name], category: "imported")
  end

  def document_group_from_metadata_params(metadata_category)
    group_category = metadata_category == "extracted_document" ? "working" : "imported"
    find_or_create_document_group(params.dig(:project_document, :document_group_name), category: group_category)
  end

  def find_or_create_document_group(name, category:)
    normalized_name = name.to_s.strip
    return nil if normalized_name.blank?

    @project.document_groups
      .where(category: category)
      .where("LOWER(name) = LOWER(?)", normalized_name)
      .first_or_create!(name: normalized_name, category: category)
  end
end
