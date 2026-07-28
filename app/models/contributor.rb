class Contributor < ApplicationRecord
  before_validation :normalize_company_name

  has_many :contributor_type_assignments, dependent: :destroy
  has_many :contributor_types, -> { order(:name) }, through: :contributor_type_assignments
  has_many :project_contributors, dependent: :restrict_with_error
  has_many :projects, through: :project_contributors
  has_many :project_issues, dependent: :restrict_with_error
  has_one :portal_user, -> { where(role: :client) }, class_name: "User", dependent: :nullify

  validates :company_name, presence: true, uniqueness: { case_sensitive: false }

  def contributor_type_names
    contributor_types.map(&:name)
  end

  def distinct_project_count
    project_contributors.map(&:project_id).uniq.size
  end

  private

  def normalize_company_name
    self.company_name = company_name.to_s.strip.presence
  end
end
