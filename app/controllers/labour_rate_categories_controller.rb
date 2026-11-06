class LabourRateCategoriesController < ApplicationController
  before_action :require_admin!
  before_action :set_category, only: [ :edit, :update, :destroy ]

  def index
    @new_category = LabourRateCategory.new(position: (LabourRateCategory.maximum(:position) || -1) + 1)
    load_categories
  end

  def create
    @new_category = LabourRateCategory.new(category_params)
    if @new_category.save
      redirect_to labour_rate_categories_path, notice: "Rate category added to client schedules."
    else
      load_categories
      render :index, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    if @category.update(category_params)
      redirect_to labour_rate_categories_path, notice: "Rate category updated."
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    if @category.active?
      redirect_to labour_rate_categories_path, alert: "Remove this category before permanently deleting it."
      return
    end

    if @category.destroy
      redirect_to labour_rate_categories_path, notice: "Rate category permanently deleted."
    else
      redirect_to labour_rate_categories_path, alert: "This category is in use and cannot be deleted. Remove it from client schedules instead."
    end
  rescue ActiveRecord::InvalidForeignKey
    redirect_to labour_rate_categories_path, alert: "This category is in use and cannot be deleted. Remove it from client schedules instead."
  end

  private

  def set_category
    @category = LabourRateCategory.find(params[:id])
  end

  def category_params
    params.require(:labour_rate_category).permit(:code, :name, :section, :position, :subheading, :active)
  end

  def load_categories
    @sections = LabourRateSection.ordered
    section_positions = @sections.pluck(:name).each_with_index.to_h
    @categories = LabourRateCategory.ordered.sort_by { |category| [ section_positions.fetch(category.section), category.position, category.code ] }
    @rate_counts = LabourRate.group(:category_key).count
  end
end
