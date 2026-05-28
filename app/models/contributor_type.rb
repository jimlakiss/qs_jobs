class ContributorType < ApplicationRecord
  has_many :contributor_type_assignments, dependent: :restrict_with_error
  has_many :contributors, -> { distinct.order(:company_name) }, through: :contributor_type_assignments

  validates :name, presence: true, uniqueness: true
end
