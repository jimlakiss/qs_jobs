class ProjectIssueMailer < ApplicationMailer
  def documents_issued(project_issue)
    @project_issue = project_issue
    @project = project_issue.project
    @recipient = project_issue.recipient_user
    @recipient_name = recipient_name

    mail(
      to: @recipient.email,
      subject: @project_issue.title
    )
  end

  private

  def recipient_name
    name = @project_issue.contributor.key_contact.to_s.strip
    return name.split.first if name.present?

    @project_issue.contributor.company_name
  end
end
