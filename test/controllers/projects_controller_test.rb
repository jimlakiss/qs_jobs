require "test_helper"

class ProjectsControllerTest < ActionDispatch::IntegrationTest
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

  test "projects default to code order from lowest to highest" do
    Project.create!(code: "2627-010")
    Project.create!(code: "2627-009")
    Project.create!(code: "2627-007")

    get projects_path

    assert_response :success
    assert_equal ["2627-007", "2627-009", "2627-010"], rendered_project_codes
  end

  test "projects can be sorted by code from highest to lowest" do
    Project.create!(code: "2627-010")
    Project.create!(code: "2627-009")
    Project.create!(code: "2627-007")

    get projects_path(sort: "code_desc")

    assert_response :success
    assert_equal ["2627-010", "2627-009", "2627-007"], rendered_project_codes
  end

  test "project document upload uses direct upload wiring" do
    project = Project.create!(code: "DOC-UPLOAD-001", address: "1 Test Street")

    get project_path(project)

    assert_response :success
    assert_includes response.body, 'data-direct-upload-url="http://www.example.com/rails/active_storage/direct_uploads"'
    assert_includes response.body, "data-direct-upload-status"
  end

  private

  def rendered_project_codes
    Nokogiri::HTML(response.body)
      .css("td.project-code-column a")
      .map { |link| link.text.tr("\u2011", "-") }
  end
end
