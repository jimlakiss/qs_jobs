class ClientSubmission < ApplicationRecord
  enum :status, { draft: 0, submitted: 1, converted: 2 }, default: :draft

  belongs_to :submitted_by, class_name: "User"
  belongs_to :contributor
  belongs_to :project, optional: true

  has_many :client_submission_documents, dependent: :destroy
  has_many_attached :documents

  validates :address, :description, presence: true
  validates :required_by, presence: true, if: :submitted?
  validate :documents_present, if: :submitted?

  scope :latest_first, -> { order(created_at: :desc) }

  def submit!
    update!(status: :submitted, submitted_at: Time.current)
  end

  private

  def documents_present
    errors.add(:documents, "must include at least one upload") unless documents.attached?
  end
end
