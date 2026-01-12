class Contributor < ApplicationRecord
  TYPES = [
    "owner",
    "builder",
    "architect",
    "interior_designer",
    "landscape_architect",
    "structural_engineer",
    "stormwater_engineer",
    "civil_engineer",
    "mechanical_engineer",
    "electrical_engineer",
    "energy_efficiency_consultant",
    "tree_arborist_consultant",
    "other"
  ].freeze

  has_many :project_contributors, dependent: :restrict_with_error
  has_many :projects, through: :project_contributors

  validates :company_name, presence: true
end