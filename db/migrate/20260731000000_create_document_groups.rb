class CreateDocumentGroups < ActiveRecord::Migration[8.1]
  def change
    create_table :document_groups do |t|
      t.references :groupable, null: false, polymorphic: true
      t.string :name, null: false

      t.timestamps
    end

    add_index :document_groups,
      "groupable_type, groupable_id, lower(btrim(name))",
      unique: true,
      name: "index_document_groups_on_groupable_and_normalized_name"

    add_reference :project_documents, :document_group, foreign_key: true
    add_reference :client_submission_documents, :document_group, foreign_key: true
  end
end
