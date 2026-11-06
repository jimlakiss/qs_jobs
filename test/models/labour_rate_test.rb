require "test_helper"

class LabourRateTest < ActiveSupport::TestCase
  test "catalogue consolidates overlapping roles and retains useful labour and supervision roles" do
    assert_equal 47, LabourRateCatalogue.entries.size
    assert_equal LabourRateCatalogue.keys.uniq, LabourRateCatalogue.keys
    names = LabourRateCatalogue.entries.pluck(:name)
    [ "Formwork Carpenter", "Wet Plastering", "Dry Plastering", "Stone Masonry", "Labourer Semi Skilled", "Labour Skilled Generic", "Structural Steel Welder", "Dogman", "Contracts Administrator", "Builder/Director" ].each do |name|
      assert_includes names, name
    end
    [ "Journeyman", "Carpenter Formworker", "Masonry & Rockwork", "Stone Mason", "Renderer", "Plasterer", "Plasterboard" ].each do |name|
      assert_not_includes names, name
    end
  end

  test "bands accept blank partial and zero values" do
    rate = users(:client).labour_rates.build(category_key: "BR", low: 0, high: 95.50)
    assert rate.valid?, rate.errors.full_messages.to_sentence
    assert rate.populated?
    rate.low = rate.high = nil
    assert rate.valid?
    assert_not rate.populated?
  end

  test "every displayed code is unique including subheadings and supervision" do
    codes = LabourRateCatalogue.entries.pluck(:code)
    assert_equal codes.uniq, codes
    codes.each { |code| assert_match(/\A[A-Z]{3}\z/, code) }
    assert_equal "LUN", LabourRateCatalogue.code_for("LH")
    assert_equal "LSS", LabourRateCatalogue.code_for("LH.labourer_semi_skilled")
    assert_equal "LSK", LabourRateCatalogue.code_for("LH.labour_skilled_generic")
    assert_equal "DOG", LabourRateCatalogue.code_for("LH.dogman")
    assert_equal "SSW", LabourRateCatalogue.code_for("MW.structural_steel_welder")
    assert_equal "SSE", LabourRateCatalogue.code_for("MW.structural_steel_erector")
    assert_equal "MET", LabourRateCatalogue.code_for("MW")
  end

  test "rejects invalid negative oversized and descending rates" do
    [ { low: "abc" }, { mid: -1 }, { high: "100000000" }, { low: 90, mid: 80 }, { low: 90, high: 80 }, { mid: 90, high: 80 } ].each do |values|
      rate = users(:client).labour_rates.build(category_key: "BR", **values)
      assert_not rate.valid?, values.inspect
    end
  end

  test "a category is unique per user and must belong to catalogue" do
    users(:client).labour_rates.create!(category_key: "BR", mid: 50)
    assert_not users(:client).labour_rates.build(category_key: "BR", mid: 60).valid?
    assert users(:other_client).labour_rates.build(category_key: "BR", mid: 60).valid?
    assert_not users(:client).labour_rates.build(category_key: "unknown", mid: 60).valid?
  end
end
