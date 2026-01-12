class ProjectsController < ApplicationController
  before_action :set_project, only: [:show, :edit, :update, :destroy]
  before_action :load_contributors, only: [:new, :edit]

  def index
  @projects = Project
    .includes(project_contributors: :contributor)
    .order(created_at: :desc)
  end

  def show
  end

  def new
    @project = Project.new
  end

  def create
    @project = Project.new(project_params)

    if @project.save
      redirect_to @project, notice: "Project created"
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit
  end

  def update
    if @project.update(project_params)
      redirect_to @project, notice: "Project updated"
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @project.destroy
    redirect_to projects_path, notice: "Project deleted"
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
end

def new
  @project = Project.new
  load_contributors
end

def edit
  load_contributors
end

def load_contributors
  @contributors_by_type =
    Contributor
      .includes(:contributor_type)
      .order(:company_name)
      .group_by { |c| c.contributor_type&.name || "Other" }
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

def assign_contributors
  return unless params[:contributors]

  params[:contributors].each do |type_id, contributor_id|
    next if contributor_id.blank?

    ProjectContributor.create!(
      project: @project,
      contributor_id: contributor_id,
      role: ContributorType.find(type_id).name.strip
    )
  end
end