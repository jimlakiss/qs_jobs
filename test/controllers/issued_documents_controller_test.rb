require "test_helper"

class IssuedDocumentsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @project = Project.create!(code: "ISSUED-CLIENT-001", address: "1 Client Street")
    @issue = @project.project_issues.create!(
      contributor: contributors(:citywide),
      recipient_user: users(:client),
      title: "Client estimate",
      description: "Issued to client",
      revision: "A",
      version: "3",
      body: "Download from the portal."
    )
    @issue.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    @issue.update!(status: :sent, sent_at: Time.current)
  end

  test "client can view issued documents" do
    sign_in users(:client)

    get issued_documents_path

    assert_response :success
    assert_includes response.body, "ISSUED-CLIENT-001 - 1 Client Street | Issued to client | Revision A Version 3"
    assert_includes response.body, "ISSUED-CLIENT-001"

    get issued_document_path(@issue)

    assert_response :success
    assert_includes response.body, "Download"
    assert_includes response.body, "test_document.txt"
  end

  test "client area shows issued documents" do
    sign_in users(:client)

    get client_area_path

    assert_response :success
    assert_includes response.body, "Issued Documents"
    assert_includes response.body, "ISSUED-CLIENT-001 - 1 Client Street | Issued to client | Revision A Version 3"
  end

  test "client cannot view documents issued to another user" do
    sign_in users(:other_client)

    get issued_document_path(@issue)

    assert_response :not_found
  end
end
