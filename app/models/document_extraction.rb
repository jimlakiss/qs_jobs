class DocumentExtraction < ApplicationRecord
  belongs_to :project
  belongs_to :document_attachment, class_name: "ActiveStorage::Attachment", foreign_key: :active_storage_attachment_id
  has_many :generated_project_documents, class_name: "ProjectDocument", dependent: :nullify

  def purge_generated_documents
    generated_project_documents.includes(:document_attachment).find_each do |project_document|
      attachment = project_document.document_attachment
      project_document.destroy
      attachment.purge
    end
  end
end
