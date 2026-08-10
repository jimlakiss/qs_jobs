class AddCategoryToDocumentGroups < ActiveRecord::Migration[8.1]
  def change
    add_column :document_groups, :category, :string, null: false, default: "imported"

    remove_index :document_groups, name: "index_document_groups_on_groupable_and_normalized_name"
    add_index :document_groups,
      "groupable_type, groupable_id, category, lower(btrim(name))",
      unique: true,
      name: "index_document_groups_on_groupable_category_and_name"
  end
end
