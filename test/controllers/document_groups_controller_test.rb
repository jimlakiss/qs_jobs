require "test_helper"

class DocumentGroupsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
    @project = Project.create!(code: "GRP-001")
    @imported_group = @project.document_groups.create!(name: "Architectural")
    @group = @project.document_groups.create!(name: "Architectural", category: "working")
  end

  test "renames a working document folder without renaming imported folder" do
    patch project_document_group_path(@project, @group),
      params: {
        tab: "working-documents",
        document_group: { name: "Architectural Rev B" }
      }

    assert_redirected_to project_path(@project, tab: "working-documents")
    assert_equal "Architectural Rev B", @group.reload.name
    assert_equal "Architectural", @imported_group.reload.name
  end

  test "allows imported and working folders with the same name" do
    assert_equal "imported", @imported_group.category
    assert_equal "working", @group.category
    assert_equal @imported_group.name, @group.name
  end

  test "downloads working document folder as zip" do
    2.times do |index|
      @project.documents.attach(
        io: StringIO.new("%PDF-1.4"),
        filename: "A#{index + 1}.pdf",
        content_type: "application/pdf"
      )
      @project.project_documents.create!(
        active_storage_attachment_id: @project.documents.attachments.last.id,
        category: "extracted_document",
        document_group: @group,
        export_kind: "working_document_pdf"
      )
    end

    get download_working_documents_project_document_group_path(@project, @group)

    assert_response :success
    assert_equal "application/zip", response.media_type
    assert_match(/Architectural\.zip/, response.headers["Content-Disposition"])
  end
end
