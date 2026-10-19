require "test_helper"

class ProjectTest < ActiveSupport::TestCase
  test "new jobs start in the first status and are current" do
    project = Project.create!(code: "NEW-JOB")
    assert_equal project_statuses(:entered), project.project_status
    assert_not project.archived?
  end

  test "earliest linked client date is used unless a scheduled date is set" do
    project = Project.create!(code: "CLIENT-DATES")
    [ 8, 3 ].each do |days|
      users(:client).client_submissions.create!(
        project: project, contributor: contributors(:citywide), address: "Site",
        description: "Drawings", required_by: Date.current + days.days
      )
    end
    assert_equal Date.current + 3.days, project.client_due_date
    assert_equal project.client_due_date, project.completion_due_date
    assert_equal [ project ], Project.completion_order.to_a
    project.update!(due_date: Date.current + 10.days)
    assert_equal Date.current + 10.days, project.completion_due_date
    assert_equal Date.current + 3.days, project.client_due_date
  end

  test "duration plans backwards from a deadline excluding weekends" do
    project = Project.create!(code: "BACKWARDS", due_date: Date.new(2026, 10, 26), duration_days: 3)
    assert_equal Date.new(2026, 10, 22), project.scheduled_start_date
    assert_equal Date.new(2026, 10, 26), project.scheduled_finish_date
  end

  test "duration plans forwards from a weekday and recalculates when duration changes" do
    project = Project.create!(code: "FORWARDS", schedule_from: "start_date",
      start_date: Date.new(2026, 10, 22), duration_days: 3)
    assert_equal Date.new(2026, 10, 26), project.due_date
    assert_equal project.start_date, project.scheduled_start_date
    project.update!(duration_days: 7)
    assert_equal Date.new(2026, 10, 30), project.reload.due_date
    project.update!(start_date: Date.new(2026, 10, 23))
    assert_equal Date.new(2026, 11, 2), project.reload.due_date
  end

  test "one day starts and finishes on the same weekday" do
    project = Project.create!(code: "ONE-DAY", schedule_from: "start_date",
      start_date: Date.new(2026, 10, 23), duration_days: 1)
    assert_equal project.start_date, project.due_date
    project.update!(schedule_from: "due_date")
    assert_equal project.due_date, project.scheduled_start_date
  end

  test "weekend client deadlines plan work to finish Friday without changing the request" do
    project = Project.create!(code: "CLIENT-WEEKEND", duration_days: 3)
    submission = users(:client).client_submissions.create!(
      project: project, contributor: contributors(:citywide), address: "Site",
      description: "Drawings", required_by: Date.new(2026, 10, 25))
    assert_equal Date.new(2026, 10, 21), project.scheduled_start_date
    assert_equal Date.new(2026, 10, 23), project.scheduled_finish_date
    assert_equal submission.required_by, project.completion_due_date
    assert_nil project.due_date

    submission.update!(required_by: Date.new(2026, 10, 24))
    project.reload
    assert_equal Date.new(2026, 10, 21), project.scheduled_start_date
    assert_equal Date.new(2026, 10, 24), project.completion_due_date
  end

  test "scheduling from start requires a weekday and duration" do
    project = Project.new(code: "INVALID-START", schedule_from: "start_date")
    assert_not project.valid?
    assert_includes project.errors[:start_date], "can't be blank"
    assert_includes project.errors[:duration_days], "can't be blank"
    [ Date.new(2026, 10, 24), Date.new(2026, 10, 25) ].each do |weekend|
      project.assign_attributes(start_date: weekend, duration_days: 2)
      assert_not project.valid?
      assert_includes project.errors[:start_date], "must be Monday to Friday"
    end
  end

  test "duration must be a positive whole number and remains optional for deadline scheduling" do
    project = Project.new(code: "DURATION")
    [ 0, -1, "1.5", "invalid" ].each do |duration|
      project.duration_days = duration
      assert_not project.valid?
      assert project.errors[:duration_days].any?
    end
    project.duration_days = nil
    assert project.valid?
    assert_nil project.scheduled_start_date
    assert_nil project.scheduled_finish_date
    project.duration_days = 3
    assert project.valid?
    assert_nil project.scheduled_start_date
  end

  test "clearing a scheduled deadline resumes scheduling from the client deadline" do
    project = Project.create!(code: "RESET", duration_days: 2, due_date: Date.new(2026, 10, 30))
    submission = users(:client).client_submissions.create!(
      project: project, contributor: contributors(:citywide), address: "Site",
      description: "Plans", required_by: Date.new(2026, 10, 26))
    project.update!(due_date: nil)
    assert_nil project.due_date
    assert_equal submission.required_by, project.completion_due_date
    assert_equal Date.new(2026, 10, 23), project.scheduled_start_date
  end

  test "longer schedules round trip across weeks and year boundaries" do
    [ Date.new(2026, 12, 28), Date.new(2026, 12, 29), Date.new(2026, 12, 30),
      Date.new(2026, 12, 31), Date.new(2027, 1, 1) ].each do |start|
      [ 5, 6, 10, 12, 30 ].each do |duration|
        project = Project.new(code: "ROUNDTRIP", schedule_from: "start_date", start_date: start, duration_days: duration)
        assert project.valid?
        expected_days = (start..project.due_date).count { |date| !date.saturday? && !date.sunday? }
        assert_equal duration, expected_days
        project.schedule_from = "due_date"
        assert_equal start, project.scheduled_start_date
      end
    end
  end

  test "requires a code" do
    project = Project.new

    assert_not project.valid?
    assert_includes project.errors[:code], "can't be blank"
  end

  test "requires a unique code regardless of case or whitespace" do
    Project.create!(code: "JOB-001")

    duplicate = Project.new(code: " job-001 ")

    assert_not duplicate.valid?
    assert_includes duplicate.errors[:code], "has already been taken"
  end

  test "normalizes code whitespace before saving" do
    project = Project.create!(code: " JOB-002 ")

    assert_equal "JOB-002", project.code
  end
end
