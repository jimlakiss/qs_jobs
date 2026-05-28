class ContributorTypeAssignment < ApplicationRecord
  belongs_to :contributor
  belongs_to :contributor_type

  validates :contributor_id, uniqueness: { scope: :contributor_type_id }
end
