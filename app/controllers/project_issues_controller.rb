class ProjectIssuesController < ApplicationController
  before_action :require_admin!
  before_action :set_project
  before_action :set_project_issue, only: [:show]

  def new
    @project_issue = @project.project_issues.new(description: "Estimate Issue")
    load_recipients
  end

  def create
    ProjectIssue.transaction do
      @project_issue = @project.project_issues.new(project_issue_params.except(:documents))
      assign_contributor_from_recipient
      @project_issue.save!
      @project_issue.documents.attach(project_issue_params[:documents])
      @project_issue.send!
    end

    ProjectIssueMailer.documents_issued(@project_issue).deliver_later

    redirect_to project_path(@project), notice: "Documents issued to client"
  rescue ActiveRecord::RecordInvalid
    load_recipients
    render :new, status: :unprocessable_entity
  end

  def show; end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end

  def set_project_issue
    @project_issue = @project.project_issues.find(params[:id])
  end

  def project_issue_params
    params.require(:project_issue).permit(:recipient_user_id, :description, :revision, :version, :body, documents: [])
  end

  def assign_contributor_from_recipient
    recipient = project_issue_recipients.find { |user| user.id == @project_issue.recipient_user_id.to_i }
    @project_issue.contributor = contributor_for_recipient(recipient)
  end

  def load_recipients
    @project_issue_recipients = project_issue_recipients
    @project_issue_recipient_labels = @project_issue_recipients.to_h { |recipient| [recipient.id, recipient_label_for(recipient)] }
    @project_issue_unavailable_contributors = unavailable_assigned_contributors
  end

  def project_issue_recipients
    @project_issue_recipients ||= begin
      (assigned_contributor_portal_users + linked_submission_uploaders)
        .uniq(&:id)
        .sort_by { |user| recipient_label_for(user).downcase }
    end
  end

  def assigned_contributors
    @assigned_contributors ||= ProjectContributor.where(project_id: @project.id).includes(:contributor).map(&:contributor).uniq(&:id)
  end

  def assigned_contributor_portal_users
    User.client.includes(:contributor).where(contributor_id: assigned_contributor_ids).to_a
  end

  def assigned_contributor_ids
    @assigned_contributor_ids ||= assigned_contributors.map(&:id)
  end

  def linked_submission_uploaders
    User.client.includes(:contributor).where(id: @project.client_submissions.select(:submitted_by_id)).to_a
  end

  def contributor_for_recipient(recipient)
    return if recipient.blank?
    return Contributor.find_by(id: recipient.contributor_id) if recipient.contributor_id.present?

    @project.client_submissions.includes(:contributor).find_by(submitted_by: recipient)&.contributor
  end

  def recipient_label_for(recipient)
    contributor = contributor_for_recipient(recipient)

    [contributor&.company_name, recipient.email].compact.join(" - ")
  end

  def unavailable_assigned_contributors
    available_contributor_ids = project_issue_recipients.filter_map { |recipient| contributor_for_recipient(recipient)&.id }

    assigned_contributors.reject { |contributor| available_contributor_ids.include?(contributor.id) }
  end
end
