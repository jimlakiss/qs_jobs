require "test_helper"

class ContributorsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
  end

  test "admin can create contributor with contributor type" do
    assert_difference("Contributor.count", 1) do
      post contributors_path, params: {
        contributor: {
          company_name: "Northside Engineers Pty Ltd",
          key_contact: "Nadia",
          email: "nadia@example.com",
          phone_number: "02 9000 0000",
          contributor_type_ids: [contributor_types(:two).id]
        }
      }
    end

    contributor = Contributor.find_by!(company_name: "Northside Engineers Pty Ltd")
    assert_redirected_to contributor_path(contributor)
    assert_equal ["Structural Engineer"], contributor.contributor_type_names
  end

  test "new contributor form shows validation errors" do
    assert_no_difference("Contributor.count") do
      post contributors_path, params: { contributor: { company_name: "" } }
    end

    assert_response :unprocessable_entity
    assert_select ".alert-danger", text: /Contributor could not be saved/
    assert_select ".alert-danger li", text: /Company name can't be blank/
  end

  test "associated projects are sorted newest to oldest by code" do
    contributor = Contributor.create!(company_name: "Sorted Project Contributor Pty Ltd")
    older_code = Project.create!(code: "2627-007", date: Date.current)
    newest_code = Project.create!(code: "2627-010", date: Date.current - 10.days)
    middle_code = Project.create!(code: "2627-009", date: Date.current - 5.days)
    ProjectContributor.create!(project: older_code, contributor: contributor, role: "Builder")
    ProjectContributor.create!(project: newest_code, contributor: contributor, role: "Architect")
    ProjectContributor.create!(project: middle_code, contributor: contributor, role: "Owner")

    get contributor_path(contributor)

    assert_response :success
    assert_equal ["2627-010", "2627-009", "2627-007"], rendered_associated_project_codes
  end

  private

  def rendered_associated_project_codes
    Nokogiri::HTML(response.body)
      .css("table tbody tr td:first-child a")
      .map(&:text)
  end
end
