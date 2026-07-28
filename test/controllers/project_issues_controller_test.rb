require "test_helper"

class ProjectIssuesControllerTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper
  include ActiveJob::TestHelper

  setup do
    sign_in users(:one)
    users(:client).update!(contributor: contributors(:citywide))
    users(:other_client).update!(contributor: contributors(:acme))
    @project = Project.create!(code: "ISSUE-001", address: "1 Issue Street", description: "Issued docs test")
    ProjectContributor.create!(project: @project, contributor: contributors(:citywide), role: "Client")
  end

  test "admin can issue documents to an assigned contributor portal user" do
    assert_difference -> { ProjectIssue.count }, 1 do
      assert_emails 1 do
        perform_enqueued_jobs do
          post project_issues_path(@project),
            params: {
              project_issue: {
                recipient_user_id: users(:client).id,
                description: "Issued estimate",
                revision: "A",
                version: "2",
                body: "Please download the attached estimate.",
                documents: [fixture_file_upload("test_document.txt", "text/plain")]
              }
            }
        end
      end
    end

    issue = ProjectIssue.last
    assert_redirected_to project_path(@project)
    assert issue.sent?
    assert_equal @project, issue.project
    assert_equal contributors(:citywide), issue.contributor
    assert_equal users(:client), issue.recipient_user
    assert_equal "ISSUE-001 - 1 Issue Street | Issued estimate | Revision A Version 2", issue.title
    assert issue.documents.attached?
    assert_equal ["client@example.com"], ActionMailer::Base.deliveries.last.to
    assert_equal "ISSUE-001 - 1 Issue Street | Issued estimate | Revision A Version 2", ActionMailer::Base.deliveries.last.subject
    assert_includes ActionMailer::Base.deliveries.last.body.encoded, issued_document_url(issue, host: "example.com")
  end

  test "admin can issue documents without revision or version suffix" do
    assert_difference -> { ProjectIssue.count }, 1 do
      perform_enqueued_jobs do
        post project_issues_path(@project),
          params: {
            project_issue: {
              recipient_user_id: users(:client).id,
              description: "Issued estimate",
              revision: "",
              version: "",
              body: "Please download the attached estimate.",
              documents: [fixture_file_upload("test_document.txt", "text/plain")]
            }
          }
      end
    end

    issue = ProjectIssue.last
    assert_redirected_to project_path(@project)
    assert_equal "ISSUE-001 - 1 Issue Street | Issued estimate", issue.title
  end

  test "admin can issue documents to a linked client submission uploader" do
    submission_user = User.create!(
      email: "submission-client@example.com",
      password: "password123",
      password_confirmation: "password123",
      role: :client
    )
    ClientSubmission.create!(
      submitted_by: submission_user,
      contributor: contributors(:citywide),
      project: @project,
      status: :converted,
      address: "1 Issue Street",
      description: "Converted upload",
      required_by: Date.current
    )

    get new_project_issue_path(@project)

    assert_response :success
    assert_select "option", text: /submission-client@example.com/

    assert_difference -> { ProjectIssue.count }, 1 do
      post project_issues_path(@project),
        params: {
          project_issue: {
            recipient_user_id: submission_user.id,
            description: "Issued estimate",
            body: "Please download the attached estimate.",
            documents: [fixture_file_upload("test_document.txt", "text/plain")]
          }
        }
    end

    issue = ProjectIssue.last
    assert_redirected_to project_path(@project)
    assert_equal contributors(:citywide), issue.contributor
    assert_equal submission_user, issue.recipient_user
    assert_equal "ISSUE-001 - 1 Issue Street | Issued estimate", issue.title
  end

  test "admin cannot issue documents without an attachment" do
    assert_no_difference -> { ProjectIssue.count } do
      post project_issues_path(@project),
        params: {
          project_issue: {
            recipient_user_id: users(:client).id,
            description: "Issued estimate",
            body: "Please download the attached estimate."
          }
        }
    end

    assert_response :unprocessable_entity
    assert_includes response.body, "Documents must include at least one file"
  end

  test "admin can issue documents to any assigned contributor portal user" do
    ProjectContributor.create!(project: @project, contributor: contributors(:acme), role: "Builder")

    get new_project_issue_path(@project)

    assert_response :success
    assert_select "option", text: /Acme Homes Pty Ltd/

    assert_difference -> { ProjectIssue.count }, 1 do
      post project_issues_path(@project),
        params: {
          project_issue: {
            recipient_user_id: users(:other_client).id,
            description: "Issued to builder",
            body: "Builder package.",
            documents: [fixture_file_upload("test_document.txt", "text/plain")]
          }
        }
    end

    issue = ProjectIssue.last
    assert_redirected_to project_path(@project)
    assert_equal contributors(:acme), issue.contributor
    assert_equal users(:other_client), issue.recipient_user
    assert_equal "ISSUE-001 - 1 Issue Street | Issued to builder", issue.title
  end

  test "new issue page lists assigned contributor portal users" do
    get new_project_issue_path(@project)

    assert_response :success
    assert_select "option", text: /Citywide Development Pty Ltd/
    assert_select ".page-subtitle", text: /assigned contributor/
    assert_select "form[data-controller='project-issue-title'][data-project-issue-title-prefix-value='ISSUE-001 - 1 Issue Street']"
    assert_select "input[name='title_preview'][value='ISSUE-001 - 1 Issue Street | <purpose>']"
    assert_select "input[name='title_preview'][data-project-issue-title-target='preview']"
    assert_select "input[name='project_issue[description]'][value='Estimate Issue'][data-project-issue-title-target='purpose']"
    assert_select "input[name='project_issue[revision]'][placeholder='A'][data-project-issue-title-target='revision']"
    assert_select "input[name='project_issue[version]'][placeholder='1'][data-project-issue-title-target='version']"
  end

  test "new issue page shows assigned contributors without portal access" do
    contributor = Contributor.create!(company_name: "No Login Pty Ltd")
    ProjectContributor.create!(project: @project, contributor: contributor, role: "Architect")

    get new_project_issue_path(@project)

    assert_response :success
    assert_select ".alert-warning", text: /Assigned contributors without portal access/
    assert_select "a[href='#{contributor_path(contributor)}']", text: "No Login Pty Ltd"
  end
end
