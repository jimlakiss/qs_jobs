require "test_helper"

class ClientSubmissionsControllerTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper

  test "client can create a draft submission with documents" do
    sign_in users(:client)

    assert_difference -> { ClientSubmission.count }, 1 do
      post client_submissions_path,
        params: {
          client_submission: {
            client_reference_code: "CLIENT-001",
            address: "10 Intake Street",
            description: "Quantity surveying services for a new home",
            required_by: Date.current + 7.days,
            notes: "Please review the architectural drawings.",
            documents: [fixture_file_upload("test_document.txt", "text/plain")]
          }
        }
    end

    submission = ClientSubmission.order(:created_at).last
    assert_redirected_to client_submission_path(submission)
    assert_equal users(:client), submission.submitted_by
    assert_equal contributors(:citywide), submission.contributor
    assert submission.documents.attached?
    assert_equal "test_document.txt", submission.client_submission_documents.first.display_name
  end

  test "client submit sends notification emails" do
    sign_in users(:client)
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-001",
      contributor: contributors(:citywide),
      address: "10 Intake Street",
      description: "Quantity surveying services",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )

    assert_enqueued_emails 2 do
      post submit_client_submission_path(submission)
    end

    assert_redirected_to client_submission_path(submission)
    assert submission.reload.submitted?
    assert submission.submitted_at.present?
  end

  test "client cannot access another client's submission" do
    sign_in users(:client)
    submission = users(:other_client).client_submissions.create!(
      contributor: contributors(:acme),
      address: "Private Street",
      description: "Private submission"
    )

    get client_submission_path(submission)

    assert_redirected_to client_submissions_path
  end

  test "client submissions index shows project code as plain text for client" do
    sign_in users(:client)
    project = Project.create!(
      code: "QS-CLIENT-001",
      date: Date.current,
      address: "10 Intake Street",
      description: "Converted project"
    )
    users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-001",
      contributor: contributors(:citywide),
      project: project,
      status: :converted,
      address: "10 Intake Street",
      description: "Quantity surveying services",
      required_by: Date.current + 7.days
    )

    get client_submissions_path

    assert_response :success
    assert_select "td", text: /QS-CLIENT-001/
    assert_select "a[href='#{project_path(project)}']", count: 0
  end

  test "client submissions index links project code for admin" do
    sign_in users(:one)
    project = Project.create!(
      code: "QS-ADMIN-001",
      date: Date.current,
      address: "10 Intake Street",
      description: "Converted project"
    )
    users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-001",
      contributor: contributors(:citywide),
      project: project,
      status: :converted,
      address: "10 Intake Street",
      description: "Quantity surveying services",
      required_by: Date.current + 7.days
    )

    get client_submissions_path

    assert_response :success
    assert_select "a[href='#{project_path(project)}']", text: "QS-ADMIN-001"
  end

  test "admin can convert submitted client submission into project" do
    sign_in users(:one)
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-001",
      contributor: contributors(:citywide),
      address: "10 Intake Street",
      description: "Quantity surveying services",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    submission.update!(status: :submitted, submitted_at: Time.current)
    submission.client_submission_documents.create!(
      active_storage_attachment_id: submission.documents.attachments.first.id,
      display_name: "Architectural drawings",
      notes: "Use for measure."
    )

    assert_difference -> { Project.count }, 1 do
      post convert_client_submission_path(submission),
        params: {
          project: {
            code: "QS-2026-001",
            date: Date.current,
            address: submission.address,
            description: submission.description
          }
        }
    end

    project = Project.find_by!(code: "QS-2026-001")
    assert_redirected_to project_path(project)
    assert submission.reload.converted?
    assert_equal project, submission.project
    assert_equal "CLIENT-001", project.client_submissions.first.client_reference_code
    assert_equal contributors(:citywide), project.project_contributors.find_by(role: "Client").contributor
    assert project.documents.attached?
    assert_equal "Client portal", project.project_documents.first.source
  end

  test "admin can add submitted client submission to existing project" do
    sign_in users(:one)
    project = Project.create!(
      code: "QS-EXISTING-001",
      date: Date.current,
      address: "10 Intake Street",
      description: "Existing project"
    )
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-EXTRA-001",
      contributor: contributors(:citywide),
      address: "10 Intake Street",
      description: "Extra drawings",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    submission.update!(status: :submitted, submitted_at: Time.current)

    assert_no_difference -> { Project.count } do
      post attach_to_project_client_submission_path(submission),
        params: {
          client_submission: {
            project_id: project.id
          }
        }
    end

    assert_redirected_to project_path(project)
    assert submission.reload.converted?
    assert_equal project, submission.project
    assert project.documents.attached?
    assert_equal "Client portal", project.project_documents.first.source
  end

  test "admin can convert draft client submission into project" do
    sign_in users(:one)
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-DRAFT-001",
      contributor: contributors(:citywide),
      address: "10 Draft Street",
      description: "Draft package",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )

    assert_difference -> { Project.count }, 1 do
      post convert_client_submission_path(submission),
        params: {
          project: {
            code: "QS-DRAFT-001",
            date: Date.current,
            address: submission.address,
            description: submission.description
          }
        }
    end

    assert_redirected_to project_path(Project.find_by!(code: "QS-DRAFT-001"))
    assert submission.reload.converted?
  end
end
