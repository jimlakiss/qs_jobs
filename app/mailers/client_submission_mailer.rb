class ClientSubmissionMailer < ApplicationMailer
  def project_uploaded(client_submission, recipient:)
    @client_submission = client_submission

    mail(
      to: recipient,
      subject: "New project added to the iQs Jobs portal: #{client_reference}"
    )
  end

  def self.internal_recipients
    ENV.fetch("CLIENT_SUBMISSIONS_EMAIL", "from@example.com")
  end

  private

  def client_reference
    @client_submission.client_reference_code.presence || @client_submission.address.to_s.truncate(60)
  end
end
