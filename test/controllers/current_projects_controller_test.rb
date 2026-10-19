require "test_helper"

class CurrentProjectsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
  end

  test "root shows current board with every status and accurate counts" do
    first = Project.create!(code: "CURRENT", due_date: Date.current + 1.day)
    Project.create!(code: "ARCHIVED", archived_at: Time.current)
    get root_path
    assert_response :success
    assert_select "article#project-#{first.id}", count: 1
    assert_select ".project-board-card", count: 1
    assert_select ".project-board-column", count: 5
    assert_select ".project-board-heading .badge", text: "1", count: 1
    assert_select ".project-board-heading .badge", text: "0", count: 4
    assert_not_includes response.body, "ARCHIVED"
  end

  test "scheduled dates order jobs with undated jobs last" do
    undated = Project.create!(code: "A")
    later = Project.create!(code: "B", due_date: Date.current + 5.days)
    earlier = Project.create!(code: "C", due_date: Date.current + 1.day)
    get current_projects_path
    assert_response :success
    assert_select ".project-board-card" do |cards|
      assert_equal [ earlier, later, undated ].map { |project| "project-#{project.id}" }, cards.map { |card| card["id"] }
    end
  end

  test "schedule moves job and changes its completion order without changing client date" do
    project = Project.create!(code: "MOVE")
    submission = users(:client).client_submissions.create!(
      project: project, contributor: contributors(:citywide),
      address: "Test Street", description: "Plans", required_by: Date.current + 3.days
    )
    other = Project.create!(code: "OTHER", due_date: Date.current + 2.days,
      project_status: project_statuses(:approved))
    patch schedule_project_path(project), params: { project: {
      project_status_id: project_statuses(:approved).id, due_date: Date.current + 1.day
    } }
    assert_redirected_to current_projects_path
    assert_equal project_statuses(:approved), project.reload.project_status
    assert_equal Date.current + 3.days, submission.reload.required_by
    assert_equal [ project, other ], Project.current.completion_order.to_a

    patch schedule_project_path(project), params: { project: { due_date: "" } }
    assert_nil project.reload.due_date
    assert_equal submission.required_by, project.completion_due_date
    assert_equal [ other, project ], Project.current.completion_order.to_a
  end

  test "card scheduling calculates weekday dates and keeps due date ordering" do
    project = Project.create!(code: "SCHEDULE-START")
    later = Project.create!(code: "SCHEDULE-LATER", due_date: Date.new(2026, 10, 27))
    patch schedule_project_path(project), params: { project: {
      schedule_from: "start_date", start_date: "2026-10-22", duration_days: 3
    } }
    assert_redirected_to current_projects_path
    assert_equal 3, project.reload.duration_days
    assert_equal Date.new(2026, 10, 26), project.due_date
    assert_equal [ project, later ], Project.current.completion_order.to_a
    get current_projects_path
    assert_response :success
    assert_select "#project-#{project.id}" do
      assert_select "p", text: "3 working days duration"
      assert_select "p", text: "Start 22 Oct 2026"
      assert_select "input[name='project[start_date]']:not([disabled])"
      assert_select "input[name='project[due_date]'][disabled]"
    end

    patch schedule_project_path(project), params: { project: {
      schedule_from: "due_date", due_date: "2026-10-30", duration_days: 3
    } }
    assert_equal Date.new(2026, 10, 28), project.reload.scheduled_start_date
    assert_equal [ later, project ], Project.current.completion_order.to_a
  end

  test "invalid card schedule does not change the stored schedule" do
    project = Project.create!(code: "BAD-SCHEDULE", duration_days: 2, due_date: Date.new(2026, 10, 26))
    patch schedule_project_path(project), params: { project: {
      schedule_from: "start_date", start_date: "2026-10-24", duration_days: 3
    } }
    assert_redirected_to current_projects_path
    assert_match /Monday to Friday/, flash[:alert]
    assert_equal 2, project.reload.duration_days
    assert_equal "due_date", project.schedule_from
    assert_equal Date.new(2026, 10, 26), project.due_date
  end

  test "archive moves a project to the register and restore returns it to the board" do
    project = Project.create!(code: "ARCHIVE-ME", project_status: project_statuses(:done))
    active = Project.create!(code: "STILL-ACTIVE")
    patch archive_project_path(project)
    assert_redirected_to projects_path
    assert project.reload.archived?
    get projects_path
    assert_select "td.project-code-column a[href='#{project_path(project)}']", count: 1
    assert_select "a[href='#{project_path(active)}']", count: 0
    patch restore_project_path(project)
    assert_redirected_to current_projects_path
    assert_not project.reload.archived?
    assert_equal project_statuses(:done), project.project_status
  end

  test "invalid status does not remove a project from the board" do
    project = Project.create!(code: "INVALID")
    patch schedule_project_path(project), params: { project: { project_status_id: -1 } }
    assert_redirected_to current_projects_path
    assert_equal project_statuses(:entered), project.reload.project_status
    assert_match /Status must exist/i, flash[:alert]
  end

  test "admin login lands on current projects" do
    sign_out :user
    post user_session_path, params: { user: { email: users(:one).email, password: "password123" } }
    assert_redirected_to current_projects_path
    sign_out :user
    post user_session_path, params: { user: { email: users(:client).email, password: "password123" } }
    assert_redirected_to client_area_path
  end

  test "clients keep their portal landing page and cannot access or change the board" do
    project = Project.create!(code: "PRIVATE")
    sign_out :user
    sign_in users(:client)
    get root_path
    assert_redirected_to client_area_path
    patch schedule_project_path(project), params: { project: { due_date: Date.current } }
    assert_redirected_to client_area_path
    assert_nil project.reload.due_date
    patch archive_project_path(project)
    assert_redirected_to client_area_path
    assert_not project.reload.archived?
    patch restore_project_path(project)
    assert_redirected_to client_area_path
    get project_statuses_path
    assert_redirected_to client_area_path
    patch project_status_path(project_statuses(:entered)), params: { project_status: { name: "Hacked" } }
    assert_redirected_to client_area_path
    assert_equal "Entered - Not Quoted", project_statuses(:entered).reload.name
  end
end
