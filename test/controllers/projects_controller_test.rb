require "test_helper"

class ProjectsControllerTest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper

  setup do
    sign_in users(:one)
  end

  test "does not create a project with a duplicate code" do
    Project.create!(code: "JOB-001")

    assert_no_difference -> { Project.count } do
      post projects_path,
        params: {
          project: {
            code: " job-001 ",
            address: "Another site address"
          }
        }
    end

    assert_response :unprocessable_entity
    assert_includes response.body, "Code has already been taken"
  end

  test "projects default to code order from highest to lowest" do
    Project.create!(code: "2627-010")
    Project.create!(code: "2627-009")
    Project.create!(code: "2627-007")

    get projects_path

    assert_response :success
    assert_equal ["2627-010", "2627-009", "2627-007"], rendered_project_codes
  end

  test "projects can be sorted by code from highest to lowest" do
    Project.create!(code: "2627-010")
    Project.create!(code: "2627-009")
    Project.create!(code: "2627-007")

    get projects_path(sort: "code_desc")

    assert_response :success
    assert_equal ["2627-010", "2627-009", "2627-007"], rendered_project_codes
  end

  test "projects index defaults to 25 projects per page" do
    30.times do |index|
      Project.create!(code: "9900-%03d" % (index + 1))
    end

    get projects_path

    assert_response :success
    assert_equal 25, rendered_project_codes.size
    assert_equal "9900-030", rendered_project_codes.first
    assert_equal "9900-006", rendered_project_codes.last
    assert_includes response.body, "Showing 1-25 of 30 projects"
    assert_select "a[href='#{projects_path(sort: "code_desc", per_page: 25, page: 2)}']", text: "Next"
  end

  test "projects index can show 50 projects per page" do
    30.times do |index|
      Project.create!(code: "9800-%03d" % (index + 1))
    end

    get projects_path(per_page: 50)

    assert_response :success
    assert_equal 30, rendered_project_codes.size
    assert_includes response.body, "Showing 1-30 of 30 projects"
  end

  test "projects index can move to the next page" do
    30.times do |index|
      Project.create!(code: "9700-%03d" % (index + 1))
    end

    get projects_path(page: 2)

    assert_response :success
    assert_equal ["9700-005", "9700-004", "9700-003", "9700-002", "9700-001"], rendered_project_codes
    assert_includes response.body, "Showing 26-30 of 30 projects"
  end

  test "project document upload uses direct upload wiring" do
    project = Project.create!(code: "DOC-UPLOAD-001", address: "1 Test Street")

    get project_path(project)

    assert_response :success
    assert_includes response.body, 'data-direct-upload-url="http://www.example.com/rails/active_storage/direct_uploads"'
    assert_includes response.body, "data-direct-upload-status"
  end

  test "project page renders saved measured dimensions" do
    project = Project.create!(code: "MEASURED-001", address: "1 Test Street")
    project.documents.attach(
      io: StringIO.new("%PDF-1.4"),
      filename: "areas.pdf",
      content_type: "application/pdf"
    )
    document = project.documents.first
    project.document_viewer_states.create!(
      active_storage_attachment_id: document.id,
      saved_at: Time.current,
      data: {
        pageBaseDimsByPage: {
          "1" => { width: 720, height: 720 }
        },
        sheetDetailsByPage: {
          "1" => { sheet_id: "A101" }
        },
        scaleZonesByPage: {
          "1" => [
            {
              label: "1:100",
              mpp: 100,
              vertices: [
                { x: 0, y: 0 },
                { x: 1, y: 0 },
                { x: 1, y: 1 },
                { x: 0, y: 1 }
              ]
            }
          ]
        },
        measurementsByPage: {
          "1" => [
            {
              type: "area",
              label: "FLOOR AREA",
              name: "Bedroom 1",
              points: [
                { x: 0, y: 0 },
                { x: 0.1, y: 0 },
                { x: 0.1, y: 0.1 },
                { x: 0, y: 0.1 }
              ]
            }
          ]
        }
      }
    )

    get project_path(project, tab: "measured-dimensions")

    assert_response :success
    assert_includes response.body, "Measured Dimensions"
    assert_includes response.body, "FLOOR AREA"
    assert_includes response.body, "Bedroom 1"
    assert_includes response.body, "A101"
    assert_includes response.body, "6.452 m²"
    assert_includes response.body, "10.16 m"
  end

  test "deleting project unlinks converted client submission without removing client upload" do
    project = Project.create!(code: "CLIENT-LINK-001", address: "1 Test Street")
    submission = users(:client).client_submissions.create!(
      client_reference_code: "CLIENT-LINK",
      contributor: contributors(:citywide),
      project: project,
      status: :converted,
      address: "1 Test Street",
      description: "Converted package",
      required_by: Date.current + 7.days
    )
    submission.documents.attach(
      io: file_fixture("test_document.txt").open,
      filename: "test_document.txt",
      content_type: "text/plain"
    )
    project.documents.attach(submission.documents.first.blob)

    assert_difference -> { Project.count }, -1 do
      perform_enqueued_jobs do
        delete project_path(project)
      end
    end

    assert_redirected_to projects_path
    assert_nil submission.reload.project
    assert submission.converted?
    assert submission.documents.attached?
  end

  private

  def rendered_project_codes
    Nokogiri::HTML(response.body)
      .css("td.project-code-column a")
      .map { |link| link.text.tr("\u2011", "-") }
  end
end
