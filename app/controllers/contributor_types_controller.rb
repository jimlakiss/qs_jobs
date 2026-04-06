class ContributorTypesController < ApplicationController
  before_action :set_contributor_type, only: [:show, :edit, :update, :destroy, :confirm_destroy]

  def index
    @query = params[:q].to_s.strip
    @contributor_types = ContributorType.all
    @contributor_types = @contributor_types.where("name ILIKE ?", "%#{@query}%") if @query.present?
    @contributor_types = @contributor_types.order(:name)
  end

  def show
    @contributors = @contributor_type.contributors.order(:company_name)
  end

  def new
    @contributor_type = ContributorType.new
  end

  def create
    @contributor_type = ContributorType.new(contributor_type_params)

    if @contributor_type.save
      redirect_to contributor_types_path, notice: "Contributor type created"
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def confirm_destroy
    @contributors = @contributor_type.contributors.order(:company_name)
  end

  def update
    previous_name = @contributor_type.name

    if @contributor_type.update(contributor_type_params)
      if previous_name != @contributor_type.name
        ProjectContributor.where(role: previous_name).update_all(role: @contributor_type.name)
      end

      redirect_to contributor_types_path, notice: "Contributor type updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    if @contributor_type.destroy
      redirect_to contributor_types_path, notice: "Contributor type deleted"
    else
      redirect_to contributor_types_path,
                  alert: "Cannot delete type in use"
    end
  end

  private

  def set_contributor_type
    @contributor_type = ContributorType.includes(:contributors).find(params[:id])
  end

  def contributor_type_params
    params.require(:contributor_type).permit(:name)
  end
end
