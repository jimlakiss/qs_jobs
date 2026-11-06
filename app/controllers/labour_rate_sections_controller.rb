class LabourRateSectionsController < ApplicationController
  before_action :require_admin!

  def update
    section = LabourRateSection.find(params[:id])
    if section.update(params.require(:labour_rate_section).permit(:position))
      redirect_to labour_rate_categories_path, notice: "Section order updated."
    else
      redirect_to labour_rate_categories_path, alert: section.errors.full_messages.to_sentence
    end
  end
end
