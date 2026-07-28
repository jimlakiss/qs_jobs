class ProjectIssue < ApplicationRecord
  enum :status, { draft: 0, sent: 1 }, default: :draft

  belongs_to :project
  belongs_to :contributor
  belongs_to :recipient_user, class_name: "User"

  has_many_attached :documents

  before_validation :build_title

  validates :title, :description, :recipient_user, presence: true
  validate :recipient_matches_contributor
  validate :documents_attached, if: :sent?

  scope :latest_first, -> { order(sent_at: :desc, created_at: :desc) }
  scope :available_to_clients, -> { sent.latest_first }

  def send!
    self.sent_at ||= Time.current
    self.status = :sent
    save!
  end

  private

  def build_title
    self.title = [
      project_issue_prefix,
      description.to_s.strip.presence,
      revision_version_label
    ].compact.join(" | ")
  end

  def recipient_matches_contributor
    return if recipient_user.blank? || contributor.blank?
    return if recipient_user.contributor_id == contributor_id
    return if project.client_submissions.exists?(submitted_by: recipient_user, contributor_id: contributor_id)

    errors.add(:recipient_user, "must belong to the selected contributor")
  end

  def documents_attached
    errors.add(:documents, "must include at least one file") unless documents.attached?
  end

  def project_issue_prefix
    [
      project&.code.to_s.strip.presence,
      project&.address.to_s.strip.presence
    ].compact.join(" - ").presence
  end

  def revision_version_label
    revision_label = revision.to_s.strip.presence&.then { |value| "Revision #{value}" }
    version_label = version.to_s.strip.presence&.then { |value| "Version #{value}" }

    [revision_label, version_label].compact.join(" ").presence
  end
end
