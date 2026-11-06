require "test_helper"

class LabourRateAveragesControllerTest < ActionDispatch::IntegrationTest
  test "admin sees accurate averages and separate sample counts for each band" do
    users(:client).labour_rates.create!(category_key: "BR", low: 0, mid: 60, high: 90)
    users(:other_client).labour_rates.create!(category_key: "BR", low: 40, mid: 80)
    users(:one).labour_rates.create!(category_key: "BR", low: 900, mid: 900, high: 900)
    sign_in users(:one)

    get labour_rate_averages_path

    assert_response :success
    assert_select "tr", text: /BRL.*Bricklayer.*20.00.*2.*70.00.*2.*90.00.*1/m
    assert_select "tr", text: /CAR.*Carpenter.*—.*0.*—.*0.*—.*0/m
    assert_select "input[type='number']", count: 0
    assert_select "a[href=?]", labour_rate_categories_path

    users(:other_client).labour_rates.find_by!(category_key: "BR").update!(mid: 100)
    get labour_rate_averages_path
    assert_select "tr", text: /BRL.*Bricklayer.*80.00/m
  end

  test "archived categories are excluded from averages" do
    category = LabourRateCategory.find_by!(key: "BR")
    users(:client).labour_rates.create!(category: category, mid: 60)
    category.update!(active: false)
    sign_in users(:one)

    get labour_rate_averages_path

    assert_response :success
    assert_select "th[scope='row']", text: "Bricklayer", count: 0
  end

  test "client cannot view averages and only sees their own schedule" do
    users(:other_client).labour_rates.create!(category_key: "BR", mid: 9876)
    sign_in users(:client)

    get labour_rate_averages_path
    assert_redirected_to client_area_path
    get labour_rates_path
    assert_select "a[href=?]", labour_rate_averages_path, count: 0
    assert_select "input[value='9876.0']", count: 0
  end

  test "anonymous visitors must sign in" do
    get labour_rate_averages_path
    assert_redirected_to new_user_session_path
  end
end
