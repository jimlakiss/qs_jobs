class Contributor < ApplicationRecord
  has_many :contributor_type_assignments, dependent: :destroy
  has_many :contributor_types, -> { order(:name) }, through: :contributor_type_assignments
  has_many :project_contributors, dependent: :restrict_with_error
  has_many :projects, through: :project_contributors

  validates :company_name, presence: true

  def contributor_type_names
    contributor_types.map(&:name)
  end
end
