class Project < ApplicationRecord
  before_validation :normalize_code

  has_many :project_contributors, dependent: :destroy
  has_many :contributors, through: :project_contributors
  has_many :document_extractions, dependent: :destroy
  has_many :project_documents, dependent: :destroy
  has_many_attached :documents

  validates :code, presence: true, uniqueness: { case_sensitive: false }

  scope :with_financials, -> { where.not(job_value: nil).or(where.not(fee_value: nil)) }

  private

  def normalize_code
    self.code = code.to_s.strip.presence
  end
end
