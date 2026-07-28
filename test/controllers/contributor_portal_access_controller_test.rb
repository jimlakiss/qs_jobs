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

  test "admin can create a new portal user for a contributor" do
    contributor = Contributor.create!(company_name: "Fresh Portal Pty Ltd")

    assert_enqueued_emails 1 do
      assert_difference -> { User.count }, 1 do
        post contributor_portal_access_path(contributor),
          params: {
            portal_access: {
              email: "fresh@example.com",
              password: "password123",
              project_upload_access: "1"
            }
          }
      end
    end

    assert_redirected_to contributor_path(contributor)
    assert_equal "fresh@example.com", contributor.reload.portal_user.email
    assert contributor.project_upload_access?
  end

  test "admin can remove contributor portal access" do
    contributor = contributors(:citywide)

    delete contributor_portal_access_path(contributor)

    assert_redirected_to contributor_path(contributor)
    assert_not contributor.reload.project_upload_access?
    assert_nil users(:client).reload.contributor
  end
end
