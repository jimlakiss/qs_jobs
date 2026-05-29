require "test_helper"

class DocumentExtractionsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
    @project = Project.create!(code: "EXT-001")
    @project.documents.attach(
      io: StringIO.new("%PDF-1.4"),
      filename: "drawings.pdf",
      content_type: "application/pdf"
    )
    @document = @project.documents.first
    @extraction = @project.document_extractions.create!(
      active_storage_attachment_id: @document.id,
      source_filename: "drawings.pdf",
      document_details: { project_id: "EXT-001" },
      sheets: [{ page: 1, sheet_id: "A001" }],
      extracted_at: Time.current
    )
  end

  test "deletes an extracted data record without deleting the source document" do
    assert_difference -> { @project.document_extractions.count }, -1 do
      delete project_document_extraction_path(@project, @extraction)
    end

    assert_redirected_to project_path(@project, tab: "extracted-data")
    assert @project.reload.documents.attached?
  end

  test "deletes generated extracted documents with the extraction record" do
    @project.documents.attach(
      io: StringIO.new("export"),
      filename: "export.csv",
      content_type: "text/csv"
    )
    export_attachment = @project.documents.last
    @project.project_documents.create!(
      active_storage_attachment_id: export_attachment.id,
      category: "extracted_document",
      export_kind: "extraction_csv",
      document_extraction: @extraction,
      generated_from_attachment_id: @document.id
    )

    assert_difference -> { @project.reload.documents.count }, -1 do
      assert_difference -> { @project.document_extractions.count }, -1 do
        delete project_document_extraction_path(@project, @extraction)
      end
    end

    assert_redirected_to project_path(@project, tab: "extracted-data")
    assert_not ActiveStorage::Attachment.exists?(export_attachment.id)
    assert @project.reload.documents.exists?(@document.id)
  end
end
