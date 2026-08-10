class CreateDocumentViewerStates < ActiveRecord::Migration[8.1]
  def change
    create_table :document_viewer_states do |t|
      t.references :project, null: false, foreign_key: true
      t.bigint :active_storage_attachment_id, null: false
      t.jsonb :data, null: false, default: {}
      t.datetime :saved_at

      t.timestamps

      t.index [:project_id, :active_storage_attachment_id], unique: true, name: "index_document_viewer_states_on_project_and_attachment"
      t.index :active_storage_attachment_id
    end
  end
end
