require "application_system_test_case"

class LabourRatesTest < ApplicationSystemTestCase
  test "client can sign in and save their own role rate" do
    visit new_user_session_path
    fill_in "Email", with: users(:client).email
    fill_in "Password", with: "password123"
    click_button "Log in"
    assert_text "Signed in successfully."

    visit labour_rates_path
    fill_in "Bricklayer — Mid ($/hour)", with: "75", fill_options: { clear: :backspace }
    assert_field "Bricklayer — Mid ($/hour)", with: "75"
    click_button "Save hourly rates", match: :first
    assert_text "Your hourly rate schedule has been saved."

    visit labour_rates_path
    assert_field "Bricklayer — Mid ($/hour)", with: "75.0"
    assert_equal BigDecimal("75"), users(:client).labour_rates.find_by!(category_key: "BR").mid
    assert_not users(:other_client).labour_rates.exists?(category_key: "BR")
  end

  test "admin can add a role through the category form" do
    visit new_user_session_path
    fill_in "Email", with: users(:one).email
    fill_in "Password", with: "password123"
    click_button "Log in"
    assert_text "Signed in successfully."

    visit labour_rate_categories_path
    fill_in "Code", with: "TEST.01-HIGH"
    fill_in "Role", with: "Specialist consultant"
    select "Supervision", from: "Section"
    click_button "Add category"
    assert_text "Rate category added to client schedules."
    assert_text "TEST.01-HIGH"
    assert_text "Specialist consultant"
  end
end
