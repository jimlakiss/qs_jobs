class ProjectContributor < ApplicationRecord
  belongs_to :project
  belongs_to :contributor

  # Role is now user-defined via ContributorType.name
  validates :role, presence: true
  validates :role, uniqueness: { scope: :project_id }
end