require "test_helper"

class LabourRateSectionsControllerTest < ActionDispatch::IntegrationTest
  test "admin can order sections independently of role positions across rate screens" do
    section = labour_rate_sections(:supervision)
    LabourRateSection.find_by!(name: "Trades & labour").update!(position: 10)
    sign_in users(:one)

    patch labour_rate_section_path(section), params: { labour_rate_section: { position: 0, name: "Ignored rename" } }

    assert_redirected_to labour_rate_categories_path
    assert_equal "Supervision", section.reload.name
    assert_equal "Supervision", LabourRateCatalogue.groups.keys.first
    get labour_rate_averages_path
    assert_response :success
    assert_select "h2", text: "Supervision"
    assert_operator response.body.index("Supervision</h2>"), :<, response.body.index("Trades &amp; labour</h2>")
    sign_in users(:client)
    get labour_rates_path
    assert_response :success
    assert_operator response.body.index("Supervision</h2>"), :<, response.body.index("Trades &amp; labour</h2>")
  end

  test "invalid section order is rejected" do
    section = labour_rate_sections(:supervision)
    sign_in users(:one)
    patch labour_rate_section_path(section), params: { labour_rate_section: { position: -1 } }
    assert_redirected_to labour_rate_categories_path
    assert_equal 1, section.reload.position
    assert flash[:alert].present?
  end

  test "client cannot change section order" do
    section = labour_rate_sections(:supervision)
    sign_in users(:client)
    patch labour_rate_section_path(section), params: { labour_rate_section: { position: 0 } }
    assert_redirected_to client_area_path
    assert_equal 1, section.reload.position
  end
end
