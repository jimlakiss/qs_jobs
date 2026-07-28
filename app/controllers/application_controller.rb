class ApplicationController < ActionController::Base
  before_action :authenticate_user!

  # Rails 8 defaults
  allow_browser versions: :modern
  stale_when_importmap_changes

  helper_method :admin_user?, :client_user?, :pending_client_uploads_count

  private

  def after_sign_in_path_for(resource)
    resource.client? ? client_submissions_path : super
  end

  def admin_user?
    current_user&.admin?
  end

  def client_user?
    current_user&.client?
  end

  def require_admin!
    redirect_to client_submissions_path, alert: "You do not have access to that area" unless admin_user?
  end

  def pending_client_uploads_count
    return 0 unless admin_user?

    @pending_client_uploads_count ||= ClientSubmission.pending_review.count
  end
end
