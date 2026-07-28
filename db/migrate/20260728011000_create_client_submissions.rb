class CreateClientSubmissions < ActiveRecord::Migration[8.1]
  def change
    create_table :client_submissions do |t|
      t.references :submitted_by, null: false, foreign_key: { to_table: :users }
      t.references :project, foreign_key: true
      t.string :client_reference_code
      t.text :address
      t.text :description
      t.date :required_by
      t.text :notes
      t.integer :status, null: false, default: 0
      t.datetime :submitted_at

      t.timestamps
    end

    add_index :client_submissions, :status
    add_index :client_submissions, :required_by
  end
end
