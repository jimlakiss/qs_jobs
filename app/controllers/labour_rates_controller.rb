class LabourRatesController < ApplicationController
  before_action :set_user

  def show
    prepare_schedule
  end

  def update
    # A user's schedule can only be edited by that user, including for admins.
    raise ActiveRecord::RecordNotFound unless @user == current_user

    if @user.update(params.require(:user).permit(labour_rates_attributes: [ :id, :category_key, :low, :mid, :high ]))
      redirect_to labour_rates_path, notice: "Your hourly rate schedule has been saved."
    else
      prepare_schedule
      render :show, status: :unprocessable_entity
    end
  end

  private

  def set_user
    if admin_user? && params[:user_id].blank?
      redirect_to contributors_path
      return
    end

    @user = if admin_user?
      User.client.includes(:contributor).find(params[:user_id])
    else
      current_user
    end
    @editable = @user == current_user
  end

  def prepare_schedule
    @groups = LabourRateCatalogue.groups
    @rate_averages = @editable ? LabourRate.client_averages(excluding_user: @user) : {}
    active_keys = @groups.values.flatten.pluck(:key)
    @rates_by_key = @user.labour_rates.select { |rate| active_keys.include?(rate.category_key) }.index_by(&:category_key)
    LabourRateCatalogue.entries.each do |entry|
      @rates_by_key[entry[:key]] ||= @user.labour_rates.build(category_key: entry[:key])
    end
  end
end
