class RestrictDeletionOfReferencedRateRecords < ActiveRecord::Migration[8.1]
  def change
    remove_foreign_key :labour_rates, :labour_rate_categories, column: :category_key, primary_key: :key
    add_foreign_key :labour_rates, :labour_rate_categories, column: :category_key, primary_key: :key, on_delete: :restrict

    remove_foreign_key :labour_rates, :users
    add_foreign_key :labour_rates, :users, on_delete: :restrict
    add_index :labour_rates, :category_key
  end
end
