class Project < ApplicationRecord
  has_many :project_contributors, dependent: :destroy
  has_many :contributors, through: :project_contributors
  has_many :document_extractions, dependent: :destroy
  has_many :project_documents, dependent: :destroy
  has_many_attached :documents

  scope :with_financials, -> { where.not(job_value: nil).or(where.not(fee_value: nil)) }
end
