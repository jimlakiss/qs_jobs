class CreateProjectContributors < ActiveRecord::Migration[8.1]
  def change
    create_table :project_contributors do |t|
      t.references :project, foreign_key: true
      t.references :contributor, foreign_key: true
      t.string :role
      t.timestamps
    end

    add_index :project_contributors, [:project_id, :role], unique: true
  end
end
