class Project < ApplicationRecord
  belongs_to :project_status

  before_validation :normalize_code
  before_validation :assign_initial_status, on: :create
  before_validation :calculate_scheduled_due_date

  has_many :project_contributors, dependent: :destroy
  has_many :contributors, through: :project_contributors
  has_many :document_extractions, dependent: :destroy
  has_many :document_viewer_states, dependent: :destroy
  has_many :project_documents, dependent: :destroy
  has_many :project_issues, dependent: :destroy
  has_many :document_groups, as: :groupable, dependent: :destroy
  has_many_attached :documents
  has_many :client_submissions, dependent: :nullify

  validates :code, presence: true, uniqueness: { case_sensitive: false }
  validates :duration_days, numericality: { only_integer: true, greater_than: 0 }, allow_nil: true
  validates :schedule_from, inclusion: { in: %w[due_date start_date] }
  validates :start_date, :duration_days, presence: true, if: :scheduled_from_start?
  validate :start_on_weekday

  scope :with_financials, -> { where.not(job_value: nil).or(where.not(fee_value: nil)) }

  scope :current, -> { where(archived_at: nil) }
  scope :archived, -> { where.not(archived_at: nil) }
  scope :completion_order, -> {
    order(Arel.sql("COALESCE(projects.due_date, (SELECT MIN(client_submissions.required_by) FROM client_submissions WHERE client_submissions.project_id = projects.id)) ASC NULLS LAST, LOWER(projects.code) ASC, projects.id ASC"))
  }

  def archived?
    archived_at.present?
  end

  def client_due_date
    client_submissions.filter_map(&:required_by).min
  end

  def completion_due_date
    due_date || client_due_date
  end

  def scheduled_from_start?
    schedule_from == "start_date"
  end

  def scheduled_start_date
    return start_date if scheduled_from_start?
    return unless completion_due_date && duration_days&.positive?

    shift_schedule_days(scheduled_finish_date, -(duration_days - 1))
  end

  def scheduled_finish_date
    return unless completion_due_date && duration_days&.positive?

    date = completion_due_date
    date -= 1 while date.saturday? || date.sunday?
    date
  end

  private

  def start_on_weekday
    return unless scheduled_from_start? && start_date

    errors.add(:start_date, "must be Monday to Friday") if start_date.saturday? || start_date.sunday?
  end

  # Scheduling counts the first day and excludes weekends. Whole weeks keep
  # longer durations inexpensive to calculate in either direction.
  def shift_schedule_days(date, days)
    direction = days <=> 0
    weeks, remainder = days.abs.divmod(5)
    date += direction * weeks * 7
    remainder.times do
      begin
        date += direction
      end while date.saturday? || date.sunday?
    end
    date
  end

  def calculate_scheduled_due_date
    return unless scheduled_from_start? && start_date && duration_days&.positive?
    return if start_date.saturday? || start_date.sunday?

    self.due_date = shift_schedule_days(start_date, duration_days - 1)
  end

  def assign_initial_status
    self.project_status ||= ProjectStatus.ordered.first
  end

  def normalize_code
    self.code = code.to_s.strip.presence
  end
end
