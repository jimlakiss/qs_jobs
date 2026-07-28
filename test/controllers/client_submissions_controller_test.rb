require "test_helper"

class ClientSubmissionsControllerTest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper
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

  test "client submit sends notification emails to client and internal inbox" do
    previous_internal_email = ENV["CLIENT_SUBMISSIONS_EMAIL"]
    ENV["CLIENT_SUBMISSIONS_EMAIL"] = "iqsjobs@cdconsult.net.au"

    begin
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

      assert_emails 2 do
        perform_enqueued_jobs do
          post submit_client_submission_path(submission)
        end
      end

      assert_redirected_to client_submission_path(submission)
      assert submission.reload.submitted?
      assert submission.submitted_at.present?
      assert_equal ["client@example.com", "iqsjobs@cdconsult.net.au"], ActionMailer::Base.deliveries.last(2).flat_map(&:to)
    ensure
      ENV["CLIENT_SUBMISSIONS_EMAIL"] = previous_internal_email
    end
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

  test "admin can archive and restore client submissions" do
    sign_in users(:one)
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-ARCHIVE-001",
      contributor: contributors(:citywide),
      address: "10 Archive Street",
      description: "Archive package",
      required_by: Date.current + 7.days
    )

    post archive_client_submission_path(submission)

    assert_redirected_to client_submissions_path
    assert submission.reload.archived?

    get client_submissions_path
    assert_response :success
    assert_select "a", text: "CLIENT-ARCHIVE-001", count: 0

    get client_submissions_path(archived: true)
    assert_response :success
    assert_select "a", text: "CLIENT-ARCHIVE-001"

    post unarchive_client_submission_path(submission)

    assert_redirected_to client_submissions_path(archived: true)
    assert_not submission.reload.archived?
  end

  test "admin can destroy converted client submission without destroying linked project" do
    sign_in users(:one)
    project = Project.create!(
      code: "QS-DELETE-001",
      date: Date.current,
      address: "10 Delete Street",
      description: "Converted project"
    )
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-DELETE-001",
      contributor: contributors(:citywide),
      project: project,
      status: :converted,
      address: "10 Delete Street",
      description: "Delete intake package",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    project.documents.attach(submission.documents.first.blob)

    assert_difference -> { ClientSubmission.count }, -1 do
      assert_no_difference -> { Project.count } do
        perform_enqueued_jobs do
          delete client_submission_path(submission)
        end
      end
    end

    assert_redirected_to client_submissions_path
    assert Project.exists?(project.id)
    assert project.reload.documents.attached?
  end

  test "admin can remove document from converted client submission without removing project copy" do
    sign_in users(:one)
    project = Project.create!(
      code: "QS-DOC-DELETE-001",
      date: Date.current,
      address: "10 Document Street",
      description: "Converted project"
    )
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-DOC-DELETE-001",
      contributor: contributors(:citywide),
      project: project,
      status: :converted,
      address: "10 Document Street",
      description: "Delete one file",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    attachment = submission.documents.attachments.first
    document = submission.client_submission_documents.create!(
      active_storage_attachment_id: attachment.id,
      display_name: "Document to remove"
    )
    project.documents.attach(attachment.blob)

    perform_enqueued_jobs do
      delete client_submission_document_path(submission, document)
    end

    assert_redirected_to client_submission_path(submission)
    assert_not submission.reload.documents.attached?
    assert project.reload.documents.attached?
    assert ActiveStorage::Blob.exists?(attachment.blob_id)
  end

  test "admin show renders remove button for uploaded document without existing metadata" do
    sign_in users(:one)
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-MISSING-META-001",
      contributor: contributors(:citywide),
      address: "10 Missing Metadata Street",
      description: "Legacy upload",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    submission.update!(status: :submitted, submitted_at: Time.current)

    assert_difference -> { submission.client_submission_documents.count }, 1 do
      get client_submission_path(submission)
    end

    assert_response :success
    document = submission.client_submission_documents.reload.first
    assert_equal "test_document.txt", document.display_name
    assert_select "a", text: "Download", count: 1
    assert_select "form[action='#{client_submission_document_path(submission, document)}'] button", text: "Remove"
  end

  test "admin client uploads nav shows active unconverted count" do
    sign_in users(:one)
    users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-PENDING-001",
      contributor: contributors(:citywide),
      address: "10 Pending Street",
      description: "Pending package",
      required_by: Date.current + 7.days
    )
    users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-ARCHIVED-001",
      contributor: contributors(:citywide),
      archived_at: Time.current,
      address: "10 Archived Street",
      description: "Archived package",
      required_by: Date.current + 7.days
    )

    get client_submissions_path

    assert_response :success
    assert_select "a.nav-link[href='#{client_submissions_path}'] .badge", text: "1"
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
