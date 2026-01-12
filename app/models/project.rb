class Project < ApplicationRecord
  has_many :project_contributors, dependent: :destroy
  has_many :contributors, through: :project_contributors
end