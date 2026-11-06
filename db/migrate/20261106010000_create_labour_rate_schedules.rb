class CreateLabourRateSchedules < ActiveRecord::Migration[8.1]
  def change
    create_table :labour_rates do |t|
      t.references :user, null: false, foreign_key: true
      t.string :category_key, null: false
      t.decimal :low, :mid, :high, precision: 10, scale: 2
      t.timestamps
    end
    add_index :labour_rates, [ :user_id, :category_key ], unique: true
    add_check_constraint :labour_rates,
      "(low IS NULL OR low >= 0) AND (mid IS NULL OR mid >= 0) AND (high IS NULL OR high >= 0)",
      name: "labour_rates_non_negative"
    add_check_constraint :labour_rates,
      "(low IS NULL OR mid IS NULL OR low <= mid) AND (mid IS NULL OR high IS NULL OR mid <= high) AND (low IS NULL OR high IS NULL OR low <= high)",
      name: "labour_rates_band_order"

    create_table :project_labour_rate_schedules do |t|
      t.references :project, null: false, foreign_key: true, index: { unique: true }
      t.references :source_user, foreign_key: { to_table: :users, on_delete: :nullify }
      t.string :source_name, null: false
      t.string :band, null: false, default: "mid"
      t.jsonb :rates, null: false, default: []
      t.datetime :applied_at, null: false
      t.timestamps
    end
    add_check_constraint :project_labour_rate_schedules, "band IN ('low', 'mid', 'high')", name: "project_labour_rates_valid_band"
  end
end
