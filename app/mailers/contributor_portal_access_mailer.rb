class ContributorPortalAccessMailer < ApplicationMailer
  def access_enabled(contributor, user, setup_token = nil)
    @contributor = contributor
    @user = user
    @setup_token = setup_token

    mail(
      to: @user.email,
      subject: "Project upload access enabled for iQs Jobs"
    )
  end
end
