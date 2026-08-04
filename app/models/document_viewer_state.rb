class DocumentViewerState < ApplicationRecord
  belongs_to :project
  belongs_to :document_attachment, class_name: "ActiveStorage::Attachment", foreign_key: :active_storage_attachment_id
end
