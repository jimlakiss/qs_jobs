class ContributorType < ApplicationRecord
  has_many :contributors, dependent: :restrict_with_error
  validates :name, presence: true, uniqueness: true
end