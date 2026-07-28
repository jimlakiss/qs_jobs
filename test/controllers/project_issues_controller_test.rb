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
    assert_select "input[name='title_preview'][value='ISSUE-001 - 1 Issue Street | Estimate Issue']"
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

  test "edit issue page shows revision controls and attachment actions" do
    issue = create_issued_issue

    get edit_project_issue_path(@project, issue)

    assert_response :success
    assert_select "form[action='#{project_issue_path(@project, issue)}']"
    assert_select "input[name='project_issue[revision]'][value='A']"
    assert_select "input[name='project_issue[version]'][value='1']"
    assert_select "input[name='project_issue[replace_documents]']"
    assert_select "a[data-turbo-method='delete'][href='#{document_project_issue_path(@project, issue, issue.documents.attachments.first)}']", text: "Remove"
    assert_select "form[action='#{project_issue_path(@project, issue)}'] button", text: "Delete Issue"
    assert_select "input[type='submit'][value='Save Issue']"
    assert_select "input[type='submit'][value='Save and Resend Email']"
  end

  test "project page shows full issue delete action" do
    issue = create_issued_issue

    get project_path(@project)

    assert_response :success
    assert_select "a[href='#{edit_project_issue_path(@project, issue)}']", text: "Edit"
    assert_select "form[action='#{project_issue_path(@project, issue)}'] button", text: "Delete"
  end

  test "admin can revise issue details without resending email" do
    issue = create_issued_issue

    assert_no_enqueued_emails do
      patch project_issue_path(@project, issue),
        params: {
          project_issue: {
            recipient_user_id: users(:client).id,
            description: "Revised issue",
            revision: "B",
            version: "2",
            body: "Revised body"
          },
          commit: "Save Issue"
        }
    end

    assert_redirected_to project_issue_path(@project, issue)
    issue.reload
    assert_equal "ISSUE-001 - 1 Issue Street | Revised issue | Revision B Version 2", issue.title
    assert_equal "Revised body", issue.body
  end

  test "admin can revise issue and resend email" do
    issue = create_issued_issue

    assert_enqueued_emails 1 do
      patch project_issue_path(@project, issue),
        params: {
          project_issue: {
            recipient_user_id: users(:client).id,
            description: "Revised issue",
            revision: "C",
            version: "",
            body: "Please review the revised issue."
          },
          commit: "Save and Resend Email"
        }
    end

    assert_redirected_to project_issue_path(@project, issue)
    assert_equal "ISSUE-001 - 1 Issue Street | Revised issue | Revision C", issue.reload.title
  end

  test "admin can add and replace issue attachments" do
    issue = create_issued_issue

    patch project_issue_path(@project, issue),
      params: {
        project_issue: {
          recipient_user_id: users(:client).id,
          description: issue.description,
          body: issue.body,
          documents: [fixture_file_upload("test_drawing.dwg", "application/acad")]
        },
        commit: "Save Issue"
      }

    assert_redirected_to project_issue_path(@project, issue)
    assert_equal 2, issue.reload.documents.count

    patch project_issue_path(@project, issue),
      params: {
        project_issue: {
          recipient_user_id: users(:client).id,
          description: issue.description,
          body: issue.body,
          replace_documents: "1",
          documents: [fixture_file_upload("test_drawing.dwg", "application/acad")]
        },
        commit: "Save Issue"
      }

    assert_redirected_to project_issue_path(@project, issue)
    assert_equal 1, issue.reload.documents.count
    assert_equal "test_drawing.dwg", issue.documents.first.filename.to_s
  end

  test "admin can remove issue attachment but not the final file" do
    issue = create_issued_issue
    issue.documents.attach(fixture_file_upload("test_drawing.dwg", "application/acad"))
    first_attachment = issue.documents.attachments.first

    assert_difference -> { issue.reload.documents.count }, -1 do
      delete document_project_issue_path(@project, issue, first_attachment)
    end

    assert_redirected_to edit_project_issue_path(@project, issue)

    final_attachment = issue.reload.documents.attachments.first
    assert_no_difference -> { issue.reload.documents.count } do
      delete document_project_issue_path(@project, issue, final_attachment)
    end

    assert_redirected_to edit_project_issue_path(@project, issue)
    assert_equal "An issued package must keep at least one attachment", flash[:alert]
  end

  test "admin can destroy an issued package" do
    issue = create_issued_issue

    assert_difference -> { ProjectIssue.count }, -1 do
      delete project_issue_path(@project, issue)
    end

    assert_redirected_to project_path(@project)
  end

  private

  def create_issued_issue
    issue = @project.project_issues.create!(
      contributor: contributors(:citywide),
      recipient_user: users(:client),
      description: "Issued estimate",
      revision: "A",
      version: "1",
      body: "Please download the attached estimate."
    )
    issue.documents.attach(fixture_file_upload("test_document.txt", "text/plain"))
    issue.send!
    issue
  end
end
