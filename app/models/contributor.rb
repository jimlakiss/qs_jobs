class Contributor < ApplicationRecord
  has_many :project_contributors, dependent: :restrict_with_error
  has_many :projects, through: :project_contributors
end