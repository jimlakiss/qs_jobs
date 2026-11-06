class CreateLabourRateSections < ActiveRecord::Migration[8.1]
  def change
    create_table :labour_rate_sections do |t|
      t.string :name, null: false
      t.integer :position, null: false, default: 0
      t.timestamps
    end
    add_index :labour_rate_sections, :name, unique: true
    add_check_constraint :labour_rate_sections, "position >= 0", name: "labour_rate_sections_non_negative_position"
    reversible do |direction|
      direction.up do
        execute <<~SQL
          INSERT INTO labour_rate_sections (name, position, created_at, updated_at)
          SELECT section, MIN(position), CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
          FROM labour_rate_categories GROUP BY section
        SQL
      end
    end
    add_foreign_key :labour_rate_categories, :labour_rate_sections, column: :section, primary_key: :name, on_delete: :restrict
  end
end
