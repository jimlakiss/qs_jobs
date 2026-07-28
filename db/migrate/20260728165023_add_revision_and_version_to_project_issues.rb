class AddRevisionAndVersionToProjectIssues < ActiveRecord::Migration[8.1]
  def change
    add_column :project_issues, :revision, :string, null: false, default: "X"
    add_column :project_issues, :version, :string, null: false, default: "X"
  end
end
