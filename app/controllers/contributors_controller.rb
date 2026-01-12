class ContributorsController < ApplicationController
  def index
    @contributors = Contributor.order(:company_name)
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

  private

  def contributor_params
    params.require(:contributor).permit(
      :company_name,
      :key_contact,
      :address,
      :phone_number,
      :email,
      :url,
      :notes
    )
  end
end