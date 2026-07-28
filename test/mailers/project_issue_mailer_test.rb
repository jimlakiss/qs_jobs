require "test_helper"

class ProjectIssueMailerTest < ActionMailer::TestCase
  test "documents issued email uses clean project issue template" do
    project = Project.create!(code: "2526-041", address: "iQs Demos")
    issue = project.project_issues.create!(
      contributor: contributors(:citywide),
      recipient_user: users(:client),
      description: "Test Document Issue",
      revision: "0",
      body: "This is a test to see if the client doc portal is working. Damn I hope it is!"
    )
    issue.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    issue.send!

    email = ProjectIssueMailer.documents_issued(issue)
    html = email.html_part.body.decoded
    text = email.text_part.body.decoded

    assert_equal ["client@example.com"], email.to
    assert_equal "2526-041 - iQs Demos | Test Document Issue | Revision 0", email.subject

    assert_includes html, "Hi James,"
    assert_includes html, "<strong>Project:</strong> 2526-041"
    assert_includes html, "<strong>Title:</strong> 2526-041 - iQs Demos | Test Document Issue | Revision 0"
    assert_includes html, "<strong>Description:</strong> Test Document Issue"
    assert_includes html, "<strong>Attachments:</strong> 1 file available"
    assert_includes html, "Log in to download the documents"
    assert_includes html, "Thanks,<br>The iQs Team"
    assert_includes html, "Quantity surveying and estimating grounded in real construction experience."
    assert_includes html, "NOTICE:"

    assert_includes text, "Hi James,"
    assert_includes text, "Project: 2526-041"
    assert_includes text, "Attachments: 1 file available"
    assert_includes text, "The iQs Team"
    assert_includes text, "NOTICE:"
  end
end
