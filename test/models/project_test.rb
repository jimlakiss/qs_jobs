require "test_helper"

class ProjectTest < ActiveSupport::TestCase
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
