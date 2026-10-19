require "test_helper"

class ProjectStatusesControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
  end

  test "admin can rename and reorder stages without changing assigned jobs" do
    status = project_statuses(:entered)
    project = Project.create!(code: "RENAME")
    patch project_status_path(status), params: { project_status: { name: "New Enquiry", position: 6 } }
    assert_redirected_to project_statuses_path
    assert_equal "New Enquiry", project.reload.project_status.name
    assert_equal status, ProjectStatus.ordered.last
    get project_statuses_path
    assert_response :success
  end

  test "admin can add a stage and duplicate names are rejected" do
    assert_difference "ProjectStatus.count", 1 do
      post project_statuses_path, params: { project_status: { name: "Invoiced", position: 5 } }
    end
    assert_no_difference "ProjectStatus.count" do
      post project_statuses_path, params: { project_status: { name: " invoiced ", position: 6 } }
    end
    assert_match /already been taken/, flash[:alert]
  end

  test "admin can delete an unused status" do
    status = project_statuses(:done)
    get project_statuses_path
    assert_response :success
    assert_select "form[action='#{project_status_path(status)}'] input[value='delete']"
    assert_difference "ProjectStatus.count", -1 do
      delete project_status_path(status)
    end
    assert_redirected_to project_statuses_path
    assert_not ProjectStatus.exists?(status.id)
  end

  test "deleting a used status reassigns both current and archived projects" do
    status = project_statuses(:quoted)
    replacement = project_statuses(:approved)
    current = Project.create!(code: "DELETE-CURRENT", project_status: status, due_date: Date.current + 3.days)
    archived = Project.create!(code: "DELETE-ARCHIVED", project_status: status, archived_at: 1.day.ago)
    archived_at = archived.archived_at

    get project_statuses_path
    assert_select "select#replacement_status_#{status.id}[required]" do
      assert_select "option[value='#{status.id}']", count: 0
      assert_select "option[value='#{replacement.id}']", count: 1
    end

    assert_no_difference "Project.count" do
      assert_difference "ProjectStatus.count", -1 do
        delete project_status_path(status), params: { replacement_status_id: replacement.id }
      end
    end
    assert_redirected_to project_statuses_path
    assert_equal replacement, current.reload.project_status
    assert_equal replacement, archived.reload.project_status
    assert_equal archived_at, archived.archived_at
    assert_not current.archived?
    assert_equal Date.current + 3.days, current.due_date
  end

  test "a used status requires a valid different replacement" do
    status = project_statuses(:quoted)
    project = Project.create!(code: "KEEP-STATUS", project_status: status)
    [ nil, status.id, -1 ].each do |replacement_id|
      assert_no_difference "ProjectStatus.count" do
        delete project_status_path(status), params: { replacement_status_id: replacement_id }
      end
      assert_redirected_to project_statuses_path
      assert_match /Choose another status/, flash[:alert]
      assert_equal status, project.reload.project_status
    end
  end

  test "the final remaining status cannot be deleted" do
    status = project_statuses(:entered)
    ProjectStatus.where.not(id: status.id).destroy_all
    get project_statuses_path
    assert_select "button[disabled]", text: "Delete"
    assert_no_difference "ProjectStatus.count" do
      delete project_status_path(status)
    end
    assert_redirected_to project_statuses_path
    assert_match /At least one project status must remain/, flash[:alert]
    assert_equal status, Project.create!(code: "STILL-CAN-CREATE").project_status
  end

  test "clients cannot delete a project status" do
    sign_out :user
    sign_in users(:client)
    assert_no_difference "ProjectStatus.count" do
      delete project_status_path(project_statuses(:done))
    end
    assert_redirected_to client_area_path
  end
end
