require "test_helper"

class LabourRateCategoryTest < ActiveSupport::TestCase
  test "normalizes codes and validates uniqueness across active and removed categories" do
    category = LabourRateCategory.new(code: " xyz ", name: " Test role ", section: " Trades & labour ")
    assert category.save
    assert_equal "XYZ", category.code
    assert_equal "Test role", category.name
    assert category.key.present?
    category.update!(active: false)
    assert_not LabourRateCategory.new(code: "xyz", name: "Duplicate").valid?
    [ "A B", "A_B", "A/B", "A" * 21, "" ].each do |code|
      assert_not LabourRateCategory.new(code: code, name: "Invalid").valid?
    end
  end

  test "codes accept letters numbers dots and hyphens up to twenty characters" do
    [ "A", "ABCD", "A12", "labour.01-high", "A" * 20 ].each do |code|
      category = LabourRateCategory.create!(code: code, name: "Flexible code")
      assert_equal code.upcase, category.reload.code
    end
    assert_not LabourRateCategory.new(code: "LABOUR.01-HIGH", name: "Duplicate").valid?
  end

  test "existing rates survive rename archive and restore" do
    category = LabourRateCategory.find_by!(key: "BR")
    rate = users(:client).labour_rates.create!(category_key: category.key, mid: 75)
    category.update!(code: "BRC", name: "Bricklaying")
    assert_equal "BRC", LabourRateCatalogue.code_for(rate.category_key)
    assert_equal BigDecimal("75"), rate.reload.mid
    category.update!(active: false)
    assert_not_includes LabourRateCatalogue.keys, category.key
    assert_equal BigDecimal("75"), rate.reload.mid
    assert_not rate.update(mid: 80)
    category.update!(active: true)
    assert_includes LabourRateCatalogue.keys, category.key
    assert_equal BigDecimal("75"), rate.reload.mid
  end

  test "referenced category cannot be destroyed through Rails or deleted directly" do
    category = LabourRateCategory.find_by!(key: "BR")
    rate = users(:client).labour_rates.create!(category: category, mid: 75)

    assert_not category.destroy
    assert category.errors[:base].present?
    assert_raises(ActiveRecord::InvalidForeignKey) do
      LabourRateCategory.transaction(requires_new: true) do
        LabourRateCategory.where(id: category.id).delete_all
      end
    end
    assert LabourRateCategory.exists?(category.id)
    assert_equal BigDecimal("75"), rate.reload.mid
  end

  test "a user with rates cannot be destroyed through Rails or deleted directly" do
    user = users(:two)
    rate = user.labour_rates.create!(category_key: "BR", mid: 75)

    assert_not user.destroy
    assert user.errors[:base].present?
    assert_raises(ActiveRecord::InvalidForeignKey) do
      User.transaction(requires_new: true) { User.where(id: user.id).delete_all }
    end
    assert User.exists?(user.id)
    assert_equal BigDecimal("75"), rate.reload.mid
  end
end
