class CreateProjectIssues < ActiveRecord::Migration[8.1]
  def change
    create_table :project_issues do |t|
      t.references :project, null: false, foreign_key: true
      t.references :contributor, null: false, foreign_key: true
      t.references :recipient_user, null: false, foreign_key: { to_table: :users }
      t.string :title, null: false
      t.string :description
      t.text :body
      t.integer :status, null: false, default: 0
      t.datetime :sent_at

      t.timestamps
    end

    add_index :project_issues, :status
    add_index :project_issues, :sent_at
  end
end
