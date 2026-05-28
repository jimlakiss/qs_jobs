require "test_helper"

class ContributorTypesControllerTest < ActionDispatch::IntegrationTest
  setup do
    sign_in users(:one)
    @contributor_type = contributor_types(:one)
  end

  test "should get index" do
    get contributor_types_url
    assert_response :success
  end

  test "should get new" do
    get new_contributor_type_url
    assert_response :success
  end

  test "should create contributor type" do
    assert_difference("ContributorType.count") do
      post contributor_types_url, params: { contributor_type: { name: "Civil Engineer" } }
    end

    assert_redirected_to contributor_types_url
  end

  test "should get edit" do
    get edit_contributor_type_url(@contributor_type)
    assert_response :success
  end

  test "should update contributor type" do
    patch contributor_type_url(@contributor_type), params: { contributor_type: { name: "Architectural Designer" } }

    assert_redirected_to contributor_types_url
    assert_equal "Architectural Designer", @contributor_type.reload.name
  end

  test "should destroy contributor type" do
    assert_difference("ContributorType.count", -1) do
      delete contributor_type_url(@contributor_type)
    end

    assert_redirected_to contributor_types_url
  end

  test "should get show" do
    get contributor_type_url(@contributor_type)
    assert_response :success
  end
end
