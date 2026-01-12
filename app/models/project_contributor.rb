class ProjectContributor < ApplicationRecord
  belongs_to :project
  belongs_to :contributor

  validates :role, presence: true
end