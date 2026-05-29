class CreateProjectDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :project_documents do |t|
      t.references :project, null: false, foreign_key: true
      t.bigint :active_storage_attachment_id, null: false
      t.string :category, null: false, default: "imported"
      t.datetime :received_at
      t.string :source
      t.string :received_from
      t.text :notes
      t.string :export_kind
      t.bigint :generated_from_attachment_id

      t.timestamps

      t.index :active_storage_attachment_id, unique: true
      t.index [:project_id, :category]
      t.index :received_at
    end
  end
end
