class ContributorsController < ApplicationController
  def index
  @contributors =
    if params[:contributor_type].present?
      Contributor.where(contributor_type: params[:contributor_type])
                 .order(:company_name)
    else
      Contributor.order(:company_name)
    end
  end

  def show
    @contributor = Contributor.find(params[:id])
  end

  def new
    @contributor = Contributor.new
  end

  def create
    @contributor = Contributor.new(contributor_params)

    if @contributor.save
      redirect_to @contributor, notice: "Contributor created"
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit
    @contributor = Contributor.find(params[:id])
  end

  def update
    @contributor = Contributor.find(params[:id])

    if @contributor.update(contributor_params)
      redirect_to @contributor, notice: "Contributor updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @contributor = Contributor.find(params[:id])

    if @contributor.destroy
      redirect_to contributors_path, notice: "Contributor deleted"
    else
      redirect_to @contributor, alert: "Cannot delete contributor in use"
    end
  end

  private

  def contributor_params
    params.require(:contributor).permit(
      :company_name,
      :contributor_type_id,
      :key_contact,
      :address,
      :phone_number,
      :email,
      :url,
      :notes
    )
  end
end