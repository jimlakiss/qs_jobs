class AllowFlexibleLabourRateCodes < ActiveRecord::Migration[8.1]
  def up
    remove_check_constraint :labour_rate_categories, name: "labour_rate_categories_three_letter_code"
    add_check_constraint :labour_rate_categories,
      "char_length(code) BETWEEN 1 AND 20 AND code ~ '^[A-Z0-9.-]+$'",
      name: "labour_rate_categories_valid_code"
  end

  def down
    remove_check_constraint :labour_rate_categories, name: "labour_rate_categories_valid_code"
    add_check_constraint :labour_rate_categories, "code ~ '^[A-Z]{3}$'", name: "labour_rate_categories_three_letter_code"
  end
end
