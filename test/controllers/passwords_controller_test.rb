require "test_helper"

class PasswordsControllerTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper

  test "password reset instructions are enqueued" do
    assert_enqueued_emails 1 do
      post user_password_path,
        params: {
          user: {
            email: users(:client).email
          }
        }
    end

    assert_response :redirect
    assert users(:client).reload.reset_password_token.present?
    assert users(:client).reset_password_sent_at.present?
  end
end
