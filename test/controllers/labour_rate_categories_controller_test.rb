require "test_helper"

class LabourRateCategoriesControllerTest < ActionDispatch::IntegrationTest
  setup do
    @category = LabourRateCategory.find_by!(key: "BR")
    sign_in users(:one)
  end

  test "admin can list add edit remove and restore catalogue categories" do
    get labour_rate_categories_path
    assert_response :success
    assert_select "h1", text: "Rate Categories"

    assert_difference "LabourRateCategory.count", 1 do
      post labour_rate_categories_path, params: { labour_rate_category: { code: "XYZ", name: "New trade", section: "Specialist labour", position: 60 } }
    end
    assert_redirected_to labour_rate_categories_path
    category = LabourRateCategory.find_by!(code: "XYZ")
    assert_includes LabourRateCatalogue.groups.fetch("Specialist labour").pluck(:key), category.key

    get edit_labour_rate_category_path(category)
    assert_response :success
    patch labour_rate_category_path(category), params: { labour_rate_category: { code: "XYA", name: "Renamed trade", subheading: true, key: "BR" } }
    assert_redirected_to labour_rate_categories_path
    assert_equal "Renamed trade", category.reload.name
    assert_not_equal "BR", category.key
    assert category.subheading?

    assert_no_difference "LabourRateCategory.count" do
      patch labour_rate_category_path(category), params: { labour_rate_category: { active: false } }
    end
    assert_redirected_to labour_rate_categories_path
    assert_not category.reload.active?

    patch labour_rate_category_path(category), params: { labour_rate_category: { active: true } }
    assert_redirected_to labour_rate_categories_path
    assert category.reload.active?
  end

  test "duplicate and invalid codes render errors without changing categories" do
    assert_no_difference "LabourRateCategory.count" do
      post labour_rate_categories_path, params: { labour_rate_category: { code: "brl", name: "Duplicate", position: 0 } }
    end
    assert_response :unprocessable_entity
    assert_select ".alert-danger", text: /already been taken/

    patch labour_rate_category_path(@category), params: { labour_rate_category: { code: "A/1" } }
    assert_response :unprocessable_entity
    assert_equal "BRL", @category.reload.code
  end

  test "admin catalogue edits appear in client portal without changing saved rates" do
    rate = users(:client).labour_rates.create!(category_key: @category.key, mid: 75)
    patch labour_rate_category_path(@category), params: { labour_rate_category: { code: "BRC", name: "Bricklaying" } }
    sign_in users(:client)
    get labour_rates_path
    assert_response :success
    assert_select "tr", text: /BRC.*Bricklaying/m
    assert_select "input[value='75.0']"
    assert_equal BigDecimal("75"), rate.reload.mid
  end

  test "removal hides a role and prevents stale edits while keeping amounts recoverable" do
    user = users(:client)
    rate = user.labour_rates.create!(category_key: @category.key, mid: 75)
    patch labour_rate_category_path(@category), params: { labour_rate_category: { active: false } }
    sign_in user
    get labour_rates_path
    assert_select "input[value='BR']", count: 0
    patch labour_rates_path, params: { user: { labour_rates_attributes: [ { id: rate.id, mid: 80 } ] } }
    assert_response :unprocessable_entity
    assert_equal BigDecimal("75"), rate.reload.mid
  end

  test "clients cannot manage the catalogue" do
    sign_in users(:client)
    get labour_rate_categories_path
    assert_redirected_to client_area_path
    get edit_labour_rate_category_path(@category)
    assert_redirected_to client_area_path
    assert_no_difference "LabourRateCategory.count" do
      post labour_rate_categories_path, params: { labour_rate_category: { code: "XYZ", name: "Unauthorized" } }
    end
    assert_redirected_to client_area_path
    patch labour_rate_category_path(@category), params: { labour_rate_category: { code: "XYZ" } }
    assert_redirected_to client_area_path
    delete labour_rate_category_path(@category)
    assert_redirected_to client_area_path
    assert @category.reload.active?
    assert_equal "BRL", @category.code
  end

  test "an archived rate does not prevent saving another active role" do
    user = users(:client)
    rate = user.labour_rates.create!(category_key: @category.key, mid: 75)
    patch labour_rate_category_path(@category), params: { labour_rate_category: { active: false } }
    sign_in user
    get labour_rates_path
    patch labour_rates_path, params: { user: { labour_rates_attributes: [ { category_key: "CA", mid: 85 } ] } }
    assert_redirected_to labour_rates_path
    assert_equal BigDecimal("85"), user.labour_rates.find_by!(category_key: "CA").mid
    assert_equal BigDecimal("75"), rate.reload.mid
  end
  test "admin can permanently delete an unused category" do
    @category.update!(active: false)
    assert_difference "LabourRateCategory.count", -1 do
      delete labour_rate_category_path(@category)
    end
    assert_redirected_to labour_rate_categories_path
    assert_not LabourRateCategory.exists?(@category.id)
  end

  test "permanent deletion refuses categories with client rates" do
    rate = users(:client).labour_rates.create!(category: @category, mid: 75)
    @category.update!(active: false)
    assert_no_difference "LabourRateCategory.count" do
      delete labour_rate_category_path(@category)
    end
    assert_redirected_to labour_rate_categories_path
    assert_match(/in use/, flash[:alert])
    assert_equal BigDecimal("75"), rate.reload.mid
    follow_redirect!
    assert_select "button[disabled]", text: "Delete"
  end

  test "active categories have remove only and cannot be permanently deleted" do
    get labour_rate_categories_path
    assert_response :success
    assert_select "tr", text: /BRL.*Bricklayer/m do
      assert_select "button", text: "Remove"
      assert_select "button", text: "Delete", count: 0
    end
    assert_no_difference "LabourRateCategory.count" do
      delete labour_rate_category_path(@category)
    end
    assert_redirected_to labour_rate_categories_path
    assert_match(/Remove this category before/, flash[:alert])
  end
end
