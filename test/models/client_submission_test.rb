require "test_helper"

class ClientSubmissionTest < ActiveSupport::TestCase
  test "submitted submissions require documents" do
    submission = users(:client).client_submissions.build(
      contributor: contributors(:citywide),
      address: "10 Intake Street",
      description: "Quantity surveying services",
      required_by: Date.current + 7.days,
      status: :submitted
    )

    assert_not submission.valid?
    assert_includes submission.errors[:documents], "must include at least one upload"
  end

  test "validation messages use client-facing field names" do
    submission = users(:client).client_submissions.build(
      contributor: contributors(:citywide),
      status: :submitted
    )

    assert_not submission.valid?
    assert_includes submission.errors.full_messages, "Project address can't be blank"
    assert_includes submission.errors.full_messages, "Project details can't be blank"
    assert_includes submission.errors.full_messages, "Required by can't be blank"
    assert_includes submission.errors.full_messages, "Documents must include at least one upload"
  end
end
