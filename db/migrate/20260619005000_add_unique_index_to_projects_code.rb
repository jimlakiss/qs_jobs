class AddUniqueIndexToProjectsCode < ActiveRecord::Migration[8.1]
  def change
    add_index :projects,
      "lower(btrim(code))",
      unique: true,
      name: "index_projects_on_normalized_code_unique"
  end
end
