class ContributorPortalAccessController < ApplicationController
  before_action :require_admin!
  before_action :set_contributor

  def create
    was_enabled = @contributor.project_upload_access?
    previous_user = @contributor.portal_user
    user = find_or_build_portal_user

    if save_portal_access(user)
      send_access_enabled_email(user) if should_send_access_enabled_email?(was_enabled, previous_user, user)
      redirect_to @contributor, notice: "Portal access updated"
    else
      redirect_to @contributor, alert: portal_access_error(user)
    end
  end

  def update
    was_enabled = @contributor.project_upload_access?
    user = @contributor.portal_user

    if user && save_portal_access(user)
      send_access_enabled_email(user) if should_send_access_enabled_email?(was_enabled, user, user)
      redirect_to @contributor, notice: "Portal access updated"
    else
      redirect_to @contributor, alert: portal_access_error(user)
    end
  end

  def destroy
    user = @contributor.portal_user

    Contributor.transaction do
      @contributor.update!(project_upload_access: false)
      user&.update!(contributor: nil)
    end

    redirect_to @contributor, notice: "Portal access removed"
  end

  private

  def set_contributor
    @contributor = Contributor.find(params[:contributor_id])
  end

  def portal_access_params
    params.require(:portal_access).permit(:email, :password, :project_upload_access)
  end

  def find_or_build_portal_user
    email = portal_access_params[:email].to_s.strip.downcase
    User.find_by(email: email) || User.new(email: email)
  end

  def save_portal_access(user)
    if user.contributor.present? && user.contributor != @contributor
      user.errors.add(:email, "is already linked to another contributor")
      return false
    end

    user.contributor = @contributor
    user.role = :client

    if portal_access_params[:password].present?
      user.password = portal_access_params[:password]
      user.password_confirmation = portal_access_params[:password]
    end

    if user.new_record? && user.password.blank?
      user.errors.add(:password, "must be set when creating a new portal user")
      return false
    end

    Contributor.transaction do
      user.save!
      @contributor.update!(project_upload_access: portal_access_enabled?)
    end

    true
  rescue ActiveRecord::RecordInvalid
    false
  end

  def portal_access_enabled?
    ActiveModel::Type::Boolean.new.cast(portal_access_params.fetch(:project_upload_access, false))
  end

  def portal_access_error(user)
    user&.errors&.full_messages&.to_sentence.presence || "Portal access could not be updated"
  end

  def should_send_access_enabled_email?(was_enabled, previous_user, user)
    portal_access_enabled? && (!was_enabled || previous_user != user || portal_access_params[:password].present?)
  end

  def send_access_enabled_email(user)
    ContributorPortalAccessMailer.access_enabled(@contributor, user).deliver_later
  end
end
