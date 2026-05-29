require "test_helper"

class ProjectDocumentsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
    @project = Project.create!(code: "DOC-001", address: "1 Test Street")
  end

  test "uploads documents to a project" do
    file = fixture_file_upload("test_document.txt", "text/plain")

    assert_difference -> { @project.documents.count }, 1 do
      post project_documents_path(@project),
        params: {
          project: {
            documents: [file],
            received_at: "2026-05-28T09:30",
            source: "Email",
            received_from: "Client",
            notes: "First issue"
          }
        }
    end

    assert_redirected_to project_path(@project)
    document = @project.reload.documents.first
    metadata = @project.project_documents.find_by!(active_storage_attachment_id: document.id)

    assert_equal "test_document.txt", document.filename.to_s
    assert_equal "imported", metadata.category
    assert_equal "Email", metadata.source
    assert_equal "Client", metadata.received_from
    assert_equal "First issue", metadata.notes
  end

  test "removes a project document" do
    @project.documents.attach(
      io: StringIO.new("temporary document"),
      filename: "temporary.txt",
      content_type: "text/plain"
    )

    document = @project.documents.first

    assert_difference -> { @project.reload.documents.count }, -1 do
      delete project_document_path(@project, document)
    end

    assert_redirected_to project_path(@project)
  end

  test "opens the viewer for pdf documents" do
    @project.documents.attach(
      io: StringIO.new("%PDF-1.4"),
      filename: "drawings.pdf",
      content_type: "application/pdf"
    )

    get viewer_project_document_path(@project, @project.documents.first)

    assert_response :success
    assert_includes response.body, "window.qsJobsDocument"
    assert_includes response.body, "drawings.pdf"
    assert_includes response.body, "/rails/active_storage/blobs/proxy/"
  end

  test "stores extracted document data" do
    @project.documents.attach(
      io: StringIO.new("%PDF-1.4"),
      filename: "drawings.pdf",
      content_type: "application/pdf"
    )

    document = @project.documents.first

    assert_difference -> { @project.document_extractions.count }, 1 do
      post save_extraction_project_document_path(@project, document),
        params: {
          extraction: {
            document_details: { project_id: "VW-001", prepared_by: "Architect" },
            sheets: [{ page: 1, sheet_id: "A001", description: "Cover sheet" }],
            regions: { sheet_id: { x: 0.1, y: 0.2, w: 0.3, h: 0.4 } },
            measurements: {},
            staging_data: [{ page: 1, filename: "A001 - Cover sheet.pdf" }]
          }
        },
        as: :json
    end

    assert_response :success

    extraction = @project.document_extractions.last
    assert_equal "VW-001", extraction.document_details["project_id"]
    assert_equal "A001", extraction.sheets.first["sheet_id"]
  end

  test "saves exported files back to project documents" do
    @project.documents.attach(
      io: StringIO.new("%PDF-1.4"),
      filename: "drawings.pdf",
      content_type: "application/pdf"
    )
    source_document = @project.documents.first
    extraction = @project.document_extractions.create!(
      active_storage_attachment_id: source_document.id,
      source_filename: "drawings.pdf",
      sheets: [{ page: 1, sheet_id: "A001" }],
      extracted_at: Time.current
    )

    export = fixture_file_upload("test_document.txt", "text/plain")

    assert_difference -> { @project.documents.count }, 1 do
      post upload_export_project_document_path(@project, source_document),
        params: { file: export, kind: "extraction_csv" }
    end

    assert_response :success
    document = @project.reload.documents.last
    metadata = @project.project_documents.find_by!(active_storage_attachment_id: document.id)

    assert_equal "test_document.txt", document.filename.to_s
    assert_equal "extracted_document", metadata.category
    assert_equal "extraction_csv", metadata.export_kind
    assert_equal source_document.id, metadata.generated_from_attachment_id
    assert_equal extraction.id, metadata.document_extraction_id
  end
end
