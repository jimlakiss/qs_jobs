class Project < ApplicationRecord
  has_many :project_contributors, dependent: :destroy
  has_many :contributors, through: :project_contributors

  scope :with_financials, -> { where.not(job_value: nil).or(where.not(fee_value: nil)) }
end
