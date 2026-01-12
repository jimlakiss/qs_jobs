class ProjectContributor < ApplicationRecord
  ROLES = [
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

  belongs_to :project
  belongs_to :contributor

  validates :role, presence: true
  validates :role, inclusion: { in: ROLES }
end