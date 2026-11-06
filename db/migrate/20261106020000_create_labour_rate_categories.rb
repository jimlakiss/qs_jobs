class CreateLabourRateCategories < ActiveRecord::Migration[8.1]
  def up
    create_table :labour_rate_categories do |t|
      t.string :key, null: false
      t.string :code, null: false
      t.string :name, null: false
      t.string :section, null: false, default: "Trades & labour"
      t.integer :position, null: false, default: 0
      t.boolean :subheading, null: false, default: false
      t.boolean :active, null: false, default: true
      t.timestamps
    end
    add_index :labour_rate_categories, :key, unique: true
    add_index :labour_rate_categories, "lower(code)", unique: true
    add_check_constraint :labour_rate_categories, "code ~ '^[A-Z]{3}$'", name: "labour_rate_categories_three_letter_code"

    category_class = Class.new(ActiveRecord::Base) { self.table_name = "labour_rate_categories" }
    definitions = JSON.parse(File.read(Rails.root.join("db/seeds/labour_rate_categories.json")))
    category_class.insert_all!(definitions.map { |row| row.merge("created_at" => Time.current, "updated_at" => Time.current) })
    add_foreign_key :labour_rates, :labour_rate_categories, column: :category_key, primary_key: :key

    # Copy removed roles only into empty schedules. Keep conflicting originals
    # on the archived category so restoring it also restores access to its rates.
    {
      "CA.journeyman" => "CA", "FW.carpenter_formworker" => "FW",
      "MA.stone_mason" => "MA", "RE.plasterer" => "RE"
    }.each do |source, target|
      execute <<~SQL
        INSERT INTO labour_rates (user_id, category_key, low, mid, high, created_at, updated_at)
        SELECT user_id, #{connection.quote(target)}, low, mid, high, created_at, updated_at
        FROM labour_rates WHERE category_key = #{connection.quote(source)}
        ON CONFLICT (user_id, category_key) DO UPDATE
        SET low = EXCLUDED.low, mid = EXCLUDED.mid, high = EXCLUDED.high, updated_at = EXCLUDED.updated_at
        WHERE labour_rates.low IS NULL AND labour_rates.mid IS NULL AND labour_rates.high IS NULL
      SQL
    end
  end

  def down
    remove_foreign_key :labour_rates, column: :category_key
    drop_table :labour_rate_categories
  end
end
