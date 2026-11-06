require "test_helper"

class LabourRatesControllerTest < ActionDispatch::IntegrationTest
  test "login is required to view or save schedules" do
    get labour_rates_path
    assert_redirected_to new_user_session_path
    patch labour_rates_path, params: { user: { labour_rates_attributes: [ { category_key: "BR", mid: "75" } ] } }
    assert_redirected_to new_user_session_path
  end

  test "client portal has a tab and all catalogue fields without creating blank records" do
    sign_in users(:client)
    get client_area_path
    assert_select "nav[aria-label='Client portal'] a[href=?]", labour_rates_path

    assert_no_difference "LabourRate.count" do
      get labour_rates_path
    end
    assert_response :success
    assert_select "input[type='number']", count: LabourRateCatalogue.entries.size * 3
    assert_select "h2", text: "Supervision"
    assert_select "input[value='FW']"
    assert_select "input[value='FW.carpenter_formworker']", count: 0
    assert_select "input[value='SU.project_manager']"
  end

  test "saves a multirow schedule and ignores blank unused roles and account changes" do
    user = users(:client)
    sign_in user
    assert_difference "LabourRate.count", 2 do
      patch labour_rates_path, params: { user: {
        role: "admin", email: "changed@example.com", labour_rates_attributes: [
          { category_key: "BR", low: "65.50", mid: "75", high: "90.25" },
          { category_key: "SU.project_manager", mid: "125" },
          { category_key: "EL", low: "", mid: "", high: "" }
        ]
      } }
    end
    assert_redirected_to labour_rates_path
    assert_equal BigDecimal("65.50"), user.labour_rates.find_by!(category_key: "BR").low
    assert user.reload.client?
    assert_equal "client@example.com", user.email
    follow_redirect!
    assert_response :success
    assert_select "input[value='65.5']"
  end

  test "updates and clears existing bands" do
    user = users(:client)
    rate = user.labour_rates.create!(category_key: "BR", low: 50, mid: 70)
    sign_in user
    patch labour_rates_path, params: { user: { labour_rates_attributes: [ { id: rate.id, category_key: "BR", low: "", mid: "", high: "" } ] } }
    assert_redirected_to labour_rates_path
    assert_not rate.reload.populated?
  end

  test "invalid row rolls back the entire schedule and retains entered values" do
    user = users(:client)
    rate = user.labour_rates.create!(category_key: "BR", mid: 70)
    sign_in user
    assert_no_difference "LabourRate.count" do
      patch labour_rates_path, params: { user: { labour_rates_attributes: [
        { id: rate.id, category_key: "BR", mid: "80" },
        { category_key: "CA", low: "100", mid: "90" }
      ] } }
    end
    assert_response :unprocessable_entity
    assert_select ".table-danger", text: /ascending order/
    assert_select "input[value='100']"
    assert_equal BigDecimal("70"), rate.reload.mid
  end

  test "client cannot read or modify another client's rates" do
    other = users(:other_client)
    rate = other.labour_rates.create!(category_key: "BR", mid: "9876.54")
    sign_in users(:client)
    get labour_rates_path(user_id: other.id)
    assert_response :success
    assert_select "input[value='9876.54']", count: 0

    patch labour_rates_path, params: { user: { labour_rates_attributes: [ { id: rate.id, mid: "10" } ] } }
    assert_response :not_found
    assert_equal BigDecimal("9876.54"), rate.reload.mid
  end

  test "admin can review a client schedule but cannot edit it" do
    user = users(:client)
    rate = user.labour_rates.create!(category_key: "BR", mid: 75)
    sign_in users(:one)
    get labour_rates_path(user_id: user.id)
    assert_response :success
    assert_select "input[type='number'][disabled]", count: LabourRateCatalogue.entries.size * 3
    assert_select "input[type='submit'][value='Save hourly rates']", count: 0

    patch labour_rates_path(user_id: user.id), params: { user: { labour_rates_attributes: [ { id: rate.id, mid: "10" } ] } }
    assert_response :not_found
    assert_equal BigDecimal("75"), rate.reload.mid
  end

  test "admin views builder rates from the contributor without a schedule selector" do
    client = users(:client)
    client.labour_rates.create!(category_key: "BR", mid: 75)
    sign_in users(:one)

    get contributor_path(client.contributor)
    assert_response :success
    assert_select "a[href=?]", labour_rates_path(user_id: client.id), text: "View Hourly Rates"

    get labour_rates_path(user_id: client.id)
    assert_response :success
    assert_select "h1", text: /#{client.contributor.company_name} · Hourly Rates/
    assert_select "select[name='user_id']", count: 0
    assert_select "nav[aria-label='Client portal']", count: 0
    assert_select "a[href=?]", contributor_path(client.contributor), text: "Back to Contributor"
    assert_select "input[type='number'][disabled][value='75.0']"
  end

  test "admin uploads have no hourly rate links or client portal tabs" do
    client = users(:client)
    client.client_submissions.create!(contributor: client.contributor, address: "1 Test Street", description: "Test job", client_reference_code: "RATE-LINK")
    sign_in users(:one)

    get client_submissions_path

    assert_response :success
    assert_select "a[href*='/labour_rates']", count: 0
    assert_select "nav[aria-label='Client portal']", count: 0
    assert_select "a[href=?]", labour_rate_categories_path, text: "Rate Categories"
  end

  test "admin must select a contributor instead of seeing an arbitrary client schedule" do
    sign_in users(:one)
    get labour_rates_path
    assert_redirected_to contributors_path
  end

  test "client sees average placeholders without saving them as their own rates" do
    users(:other_client).labour_rates.create!(category_key: "BR", low: 0, mid: 70, high: 100)
    sign_in users(:client)

    get labour_rates_path

    assert_response :success
    assert_select "input[type='number'][name$='[low]'][placeholder='0.00']:not([value])"
    assert_select "input[type='number'][name$='[mid]'][placeholder='70.00']:not([value])"
    assert_select "input[type='number'][name$='[high]'][placeholder='100.00']:not([value])"
    assert_select "input[type='number'][placeholder='Not set']"
    assert_select "#rate-average-help", text: /leaving a field blank does not save the average/

    assert_no_difference "LabourRate.count" do
      patch labour_rates_path, params: { user: { labour_rates_attributes: [ { category_key: "BR", low: "", mid: "", high: "" } ] } }
    end
    assert_redirected_to labour_rates_path
    assert_not users(:client).labour_rates.exists?(category_key: "BR")
  end
end
