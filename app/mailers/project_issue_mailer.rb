class ProjectIssueMailer < ApplicationMailer
  def documents_issued(project_issue)
    @project_issue = project_issue
    @project = project_issue.project
    @recipient = project_issue.recipient_user

    mail(
      to: @recipient.email,
      subject: @project_issue.title
    )
  end
end
