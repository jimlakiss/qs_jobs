class LabourRateAveragesController < ApplicationController
  before_action :require_admin!

  def index
    @groups = LabourRateCatalogue.groups
    @averages = LabourRate.client_averages
  end
end
