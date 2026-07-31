class DocumentGroup < ApplicationRecord
  belongs_to :groupable, polymorphic: true

  has_many :project_documents, dependent: :nullify
  has_many :client_submission_documents, dependent: :nullify

  before_validation :normalize_name

  validates :name, presence: true
  validates :name,
    uniqueness: {
      scope: [:groupable_type, :groupable_id],
      case_sensitive: false
    }

  scope :alphabetical, -> { order(Arel.sql("LOWER(name), name")) }

  private

  def normalize_name
    self.name = name.to_s.strip.presence
  end
end
