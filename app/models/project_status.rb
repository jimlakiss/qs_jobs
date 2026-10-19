class ProjectStatus < ApplicationRecord
  DEFAULT_NAMES = [
    "Entered - Not Quoted",
    "Quoted - Waiting for Approval",
    "Approved - To Be Completed",
    "Job Being Completed",
    "Done - Not Invoiced"
  ].freeze

  has_many :projects, dependent: :restrict_with_error

  normalizes :name, with: ->(name) { name.strip }
  validates :name, presence: true, uniqueness: { case_sensitive: false }
  validates :position, numericality: { only_integer: true, greater_than_or_equal_to: 0 }

  scope :ordered, -> { order(:position, :id) }

  def remove_with_replacement(replacement_id: nil)
    self.class.transaction do
      # Serialize removals so two requests cannot delete the final stages together.
      statuses = self.class.order(:id).lock.to_a
      if statuses.size <= 1
        errors.add(:base, "At least one project status must remain")
        next false
      end

      replacement = statuses.find { |status| status.id.to_s == replacement_id.to_s && status.id != id }
      if replacement.nil? && (projects.exists? || replacement_id.present?)
        errors.add(:base, "Choose another status for the assigned projects")
        next false
      end

      # Include archived jobs, retaining their archive state and scheduling dates.
      projects.update_all(project_status_id: replacement.id, updated_at: Time.current) if replacement
      destroy!
      true
    end
  end
end
