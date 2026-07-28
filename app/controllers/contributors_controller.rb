class ContributorsController < ApplicationController
  CONTRIBUTOR_SORT_OPTIONS = {
    "code" => {
      order: ->(direction) { { id: direction } }
    },
    "company" => {
      order: ->(direction) { Arel.sql("LOWER(contributors.company_name) #{sql_direction(direction)}, contributors.company_name #{sql_direction(direction)}, contributors.id ASC") }
    },
    "projects" => {
      aggregate: true,
      joins: :project_contributors,
      order: ->(direction) { Arel.sql("COUNT(DISTINCT project_contributors.project_id) #{sql_direction(direction)}, contributors.id ASC") }
    },
    "contact" => {
      order: ->(direction) { Arel.sql("LOWER(contributors.key_contact) #{sql_direction(direction)} NULLS LAST, contributors.id ASC") }
    },
    "email" => {
      order: ->(direction) { Arel.sql("LOWER(contributors.email) #{sql_direction(direction)} NULLS LAST, contributors.id ASC") }
    },
    "phone" => {
      order: ->(direction) { Arel.sql("LOWER(contributors.phone_number) #{sql_direction(direction)} NULLS LAST, contributors.id ASC") }
    },
    "types" => {
      aggregate: true,
      joins: :contributor_types,
      order: ->(direction) { Arel.sql("LOWER(MIN(contributor_types.name)) #{sql_direction(direction)} NULLS LAST, contributors.id ASC") }
    }
  }.freeze

  before_action :require_admin!
  before_action :set_contributor, only: [:show, :edit, :update, :destroy, :confirm_destroy]

  def index
    @query = params[:q].to_s.strip
    @sort = params[:sort].presence_in(CONTRIBUTOR_SORT_OPTIONS.keys) || "code"
    @direction = params[:direction].presence_in(%w[asc desc]) || "asc"
    @contributors =
      if params[:contributor_type].present?
        Contributor.joins(:contributor_types).where(contributor_types: { id: params[:contributor_type] }).distinct
      else
        Contributor.all
      end

    if @query.present?
      @contributors = @contributors.where(
        "company_name ILIKE :q OR key_contact ILIKE :q OR email ILIKE :q OR phone_number ILIKE :q",
        q: "%#{@query}%"
      )
    end

    @contributors = sort_contributors(@contributors).includes(:contributor_types, :project_contributors)
  end

  def show
    @associated_projects = @contributor.projects.distinct.order(date: :desc, code: :asc)
    @associated_job_value = @associated_projects.sum(:job_value)
    @associated_fee_value = @associated_projects.sum(:fee_value)
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

  def edit; end

  def confirm_destroy
    @associated_projects = @contributor.projects.distinct.order(date: :desc, code: :asc)
  end

  def update
    if @contributor.update(contributor_params)
      redirect_to @contributor, notice: "Contributor updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    if @contributor.destroy
      redirect_to contributors_path, notice: "Contributor deleted"
    else
      redirect_to @contributor, alert: "Cannot delete contributor in use"
    end
  end

  private

  def set_contributor
    @contributor = Contributor.includes(:contributor_types, :project_contributors, projects: :project_contributors).find(params[:id])
  end

  def contributor_params
    params.require(:contributor).permit(
      :company_name,
      :key_contact,
      :address,
      :phone_number,
      :email,
      :url,
      :notes,
      contributor_type_ids: []
    )
  end

  def sort_contributors(scope)
    sort_config = CONTRIBUTOR_SORT_OPTIONS.fetch(@sort)
    scope = scope.left_joins(sort_config[:joins]) if sort_config[:joins]
    scope = scope.group("contributors.id") if sort_config[:aggregate]
    scope.order(sort_config.fetch(:order).call(@direction))
  end

  def self.sql_direction(direction)
    direction == "desc" ? "DESC" : "ASC"
  end
end
