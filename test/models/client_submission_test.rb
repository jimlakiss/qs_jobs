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
end
