class AddDocumentExtractionToProjectDocuments < ActiveRecord::Migration[8.1]
  def change
    add_reference :project_documents, :document_extraction, foreign_key: true
  end
end
