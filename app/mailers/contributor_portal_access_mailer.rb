class ContributorPortalAccessMailer < ApplicationMailer
  def access_enabled(contributor, user)
    @contributor = contributor
    @user = user

    mail(
      to: @user.email,
      subject: "Project upload access enabled for iQs Jobs"
    )
  end
end
