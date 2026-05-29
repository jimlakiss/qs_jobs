class ProjectDocument < ApplicationRecord
  CATEGORIES = %w[imported extracted_document].freeze

  belongs_to :project
  belongs_to :document_attachment, class_name: "ActiveStorage::Attachment", foreign_key: :active_storage_attachment_id
  belongs_to :generated_from_attachment,
    class_name: "ActiveStorage::Attachment",
    optional: true
  belongs_to :document_extraction, optional: true

  validates :category, inclusion: { in: CATEGORIES }

  scope :imported, -> { where(category: "imported") }
  scope :extracted_documents, -> { where(category: "extracted_document") }
end
