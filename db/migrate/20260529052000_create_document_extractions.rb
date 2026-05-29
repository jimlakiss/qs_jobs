class CreateDocumentExtractions < ActiveRecord::Migration[8.1]
  def change
    create_table :document_extractions do |t|
      t.references :project, null: false, foreign_key: true
      t.bigint :active_storage_attachment_id, null: false
      t.jsonb :document_details, null: false, default: {}
      t.jsonb :sheets, null: false, default: []
      t.jsonb :regions, null: false, default: {}
      t.jsonb :measurements, null: false, default: {}
      t.jsonb :staging_data, null: false, default: []
      t.string :source_filename
      t.datetime :extracted_at

      t.timestamps

      t.index [:project_id, :active_storage_attachment_id], unique: true, name: "index_document_extractions_on_project_and_attachment"
      t.index :active_storage_attachment_id
    end
  end
end
