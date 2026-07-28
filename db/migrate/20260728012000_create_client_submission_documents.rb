class CreateClientSubmissionDocuments < ActiveRecord::Migration[8.1]
  def change
    create_table :client_submission_documents do |t|
      t.references :client_submission, null: false, foreign_key: true
      t.bigint :active_storage_attachment_id, null: false
      t.string :display_name
      t.text :notes

      t.timestamps
    end

    add_index :client_submission_documents,
      :active_storage_attachment_id,
      unique: true,
      name: "index_client_submission_documents_on_attachment_id"
  end
end
