class ApplicationMailer < ActionMailer::Base
  default from: ENV.fetch("MAIL_FROM", "iQs Jobs <from@example.com>")
  layout "mailer"
end
