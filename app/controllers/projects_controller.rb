class ProjectsController < ApplicationController
  before_action :set_project, only: [:show, :edit, :update]

  def index
    @projects = Project
      .includes(project_contributors: :contributor)
      .order(created_at: :desc)
  end

  def show
  end

  def new
    @project = Project.new
    load_contributors
  end

  def create
    @project = Project.new(project_params)

    if @project.save
      assign_contributors
      redirect_to @project, notice: "Project created"
    else
      load_contributors
      render :new, status: :unprocessable_entity
    end
  end

  def edit
    load_contributors
  end

  def update
    if @project.update(project_params)
      @project.project_contributors.destroy_all
      assign_contributors
      redirect_to @project, notice: "Project updated"
    else
      load_contributors
      render :edit, status: :unprocessable_entity
    end
  end

  private

  def set_project
    @project = Project.find(params[:id])
  end

  def project_params
    params.require(:project).permit(
      :code,
      :date,
      :address,
      :description,
      :job_value
    )
  end

 def load_contributors
  @contributors_by_type = Hash.new { |h, k| h[k] = [] }

  Contributor.order(:company_name).each do |c|
    key = c.contributor_type.presence || "other"
    @contributors_by_type[key] << c
  end
end

  def assign_contributors
    return unless params[:contributors]

    params[:contributors].each do |role, contributor_id|
      next if contributor_id.blank?

      ProjectContributor.create!(
        project: @project,
        contributor_id: contributor_id,
        role: role
      )
    end
  end
end