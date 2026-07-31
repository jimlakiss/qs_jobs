class ClientSubmissionDocument < ApplicationRecord
  belongs_to :client_submission
  belongs_to :document_attachment,
    class_name: "ActiveStorage::Attachment",
    foreign_key: :active_storage_attachment_id
  belongs_to :document_group, optional: true

  validates :display_name, presence: true
end
