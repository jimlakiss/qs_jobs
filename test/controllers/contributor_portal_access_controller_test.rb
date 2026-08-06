require "test_helper"

class ContributorPortalAccessControllerTest < ActionDispatch::IntegrationTest
  include ActionMailer::TestHelper

  setup do
    sign_in users(:one)
  end

  test "admin can link an existing user to contributor portal access" do
    contributor = Contributor.create!(company_name: "Portal Client Pty Ltd")
    user = User.create!(email: "portal@example.com", password: "password123")

    assert_enqueued_emails 1 do
      post contributor_portal_access_path(contributor),
        params: {
          portal_access: {
            email: user.email,
            project_upload_access: "1"
          }
        }
    end

    assert_redirected_to contributor_path(contributor)
    assert_equal user, contributor.reload.portal_user
    assert contributor.project_upload_access?
    assert user.reload.client?
  end

  test "admin re-enabling access sends setup link for an existing user with a password" do
    contributor = Contributor.create!(company_name: "Returning Portal Pty Ltd")
    user = User.create!(email: "returning@example.com", password: "password123")
    contributor.update!(project_upload_access: false)

    assert_emails 1 do
      perform_enqueued_jobs do
        post contributor_portal_access_path(contributor),
          params: {
            portal_access: {
              email: user.email,
              project_upload_access: "1"
            }
          }
      end
    end

    mail = ActionMailer::Base.deliveries.last
    assert_includes mail.text_part.body.to_s, "Set up your password:"
    assert user.reload.reset_password_token.present?
    assert user.reset_password_sent_at.present?
  end

  test "admin can create a new portal user for a contributor" do
    contributor = Contributor.create!(company_name: "Fresh Portal Pty Ltd")

    assert_enqueued_emails 1 do
      assert_difference -> { User.count }, 1 do
        post contributor_portal_access_path(contributor),
          params: {
            portal_access: {
              email: "fresh@example.com",
              project_upload_access: "1"
            }
          }
      end
    end

    assert_redirected_to contributor_path(contributor)
    user = contributor.reload.portal_user
    assert_equal "fresh@example.com", user.email
    assert_not user.password_set?
    assert user.reset_password_token.present?
    assert user.reset_password_sent_at.present?
    assert contributor.project_upload_access?
  end

  test "missing project upload access is treated as disabled" do
    contributor = Contributor.create!(company_name: "Unchecked Portal Pty Ltd")

    assert_difference -> { User.count }, 1 do
      post contributor_portal_access_path(contributor),
        params: {
          portal_access: {
            email: "unchecked@example.com"
          }
        }
    end

    assert_redirected_to contributor_path(contributor)
    assert_equal "unchecked@example.com", contributor.reload.portal_user.email
    assert_not contributor.project_upload_access?
  end

  test "new portal user can set password from access setup link" do
    contributor = Contributor.create!(company_name: "Setup Portal Pty Ltd")

    perform_enqueued_jobs do
      post contributor_portal_access_path(contributor),
        params: {
          portal_access: {
            email: "setup@example.com",
            project_upload_access: "1"
          }
        }
    end

    mail = ActionMailer::Base.deliveries.last
    setup_url = mail.text_part.body.to_s[/Set up your password: (.+)$/, 1]
    token = Rack::Utils.parse_query(URI.parse(setup_url).query)["reset_password_token"]

    sign_out users(:one)

    put user_password_path,
      params: {
        user: {
          reset_password_token: token,
          password: "password123",
          password_confirmation: "password123"
        }
      }

    user = User.find_by!(email: "setup@example.com")
    assert user.password_set?
    assert user.valid_password?("password123")
  end

  test "admin can remove contributor portal access" do
    contributor = contributors(:citywide)

    delete contributor_portal_access_path(contributor)

    assert_redirected_to contributor_path(contributor)
    assert_not contributor.reload.project_upload_access?
    assert_nil users(:client).reload.contributor
  end
end
