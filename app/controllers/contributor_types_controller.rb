class ContributorTypesController < ApplicationController
  def index
    @contributor_types = ContributorType.order(:name)
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

  def edit
    @contributor_type = ContributorType.find(params[:id])
  end

  def update
    @contributor_type = ContributorType.find(params[:id])

    if @contributor_type.update(contributor_type_params)
      redirect_to contributor_types_path, notice: "Contributor type updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @contributor_type = ContributorType.find(params[:id])

    if @contributor_type.destroy
      redirect_to contributor_types_path, notice: "Contributor type deleted"
    else
      redirect_to contributor_types_path,
                  alert: "Cannot delete type in use"
    end
  end

  private

  def contributor_type_params
    params.require(:contributor_type).permit(:name)
  end
end
