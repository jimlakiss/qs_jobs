class Contributor < ApplicationRecord
  belongs_to :contributor_type, optional: true

  has_many :project_contributors, dependent: :restrict_with_error
  has_many :projects, through: :project_contributors

  validates :company_name, presence: true
end