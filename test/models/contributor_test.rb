require "test_helper"

class ContributorTest < ActiveSupport::TestCase
  test "requires a company name" do
    contributor = Contributor.new

    assert_not contributor.valid?
    assert_includes contributor.errors[:company_name], "can't be blank"
  end

  test "requires a unique company name regardless of case or whitespace" do
    Contributor.create!(company_name: "Example Consulting")

    duplicate = Contributor.new(company_name: " example consulting ")

    assert_not duplicate.valid?
    assert_includes duplicate.errors[:company_name], "has already been taken"
  end

  test "normalizes company name whitespace before saving" do
    contributor = Contributor.create!(company_name: " Example Consulting ")

    assert_equal "Example Consulting", contributor.company_name
  end
end
