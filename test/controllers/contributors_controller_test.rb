require "test_helper"

class ContributorsControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
  end

  test "contributors default to code order" do
    first = Contributor.create!(company_name: "Sortable Zeta Pty Ltd")
    second = Contributor.create!(company_name: "Sortable Alpha Pty Ltd")
    third = Contributor.create!(company_name: "Sortable Middle Pty Ltd")

    get contributors_path(q: "Sortable")

    assert_response :success
    assert_equal [first, second, third].map { |contributor| "/#{contributor.id}" }, rendered_contributor_codes
  end

  test "contributors can be sorted by company heading" do
    Contributor.create!(company_name: "Sortable Zeta Pty Ltd")
    Contributor.create!(company_name: "Sortable Alpha Pty Ltd")
    Contributor.create!(company_name: "Sortable Middle Pty Ltd")

    get contributors_path(q: "Sortable", sort: "company", direction: "desc")

    assert_response :success
    assert_equal ["Sortable Zeta Pty Ltd", "Sortable Middle Pty Ltd", "Sortable Alpha Pty Ltd"], rendered_contributor_companies
  end

  test "contributors can be sorted by project count" do
    no_projects = Contributor.create!(company_name: "Sortable Projects None Pty Ltd")
    one_project = Contributor.create!(company_name: "Sortable Projects One Pty Ltd")
    two_projects = Contributor.create!(company_name: "Sortable Projects Two Pty Ltd")
    ProjectContributor.create!(project: Project.create!(code: "SORT-PROJECT-001"), contributor: one_project, role: "Builder")
    ProjectContributor.create!(project: Project.create!(code: "SORT-PROJECT-002"), contributor: two_projects, role: "Builder")
    ProjectContributor.create!(project: Project.create!(code: "SORT-PROJECT-003"), contributor: two_projects, role: "Owner")

    get contributors_path(q: "Sortable Projects", sort: "projects", direction: "desc")

    assert_response :success
    assert_equal [two_projects, one_project, no_projects].map(&:company_name), rendered_contributor_companies
  end

  test "contributors can be sorted by contributor type" do
    architect = Contributor.create!(company_name: "Sortable Types Architect Pty Ltd")
    engineer = Contributor.create!(company_name: "Sortable Types Engineer Pty Ltd")
    ContributorTypeAssignment.create!(contributor: engineer, contributor_type: contributor_types(:two))
    ContributorTypeAssignment.create!(contributor: architect, contributor_type: contributor_types(:one))

    get contributors_path(q: "Sortable Types", sort: "types", direction: "asc")

    assert_response :success
    assert_equal [architect.company_name, engineer.company_name], rendered_contributor_companies
  end

  test "sortable contributor headings preserve search query" do
    get contributors_path(q: "Citywide")

    assert_response :success
    company_header = Nokogiri::HTML(response.body).css("thead a").find { |link| link.text.include?("Company") }
    assert company_header
    assert_includes company_header["href"], "q=Citywide"
    assert_includes company_header["href"], "sort=company"
  end

  private

  def rendered_contributor_codes
    Nokogiri::HTML(response.body)
      .css("tbody tr td:first-child a")
      .map(&:text)
  end

  def rendered_contributor_companies
    Nokogiri::HTML(response.body)
      .css("tbody tr td:nth-child(2) a")
      .map(&:text)
  end
end
