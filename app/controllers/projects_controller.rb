class ProjectsController < ApplicationController
  before_action :set_project, only: [:show, :edit, :update, :destroy]
  before_action :load_contributors, only: [:new, :edit]

  def index
    @projects = Project.includes(project_contributors: :contributor).order(created_at: :desc)
  end

  def show; end

  def new
    @project = Project.new
  end

  def create
    @project = Project.new(project_params)

    if @project.save
      upsert_contributors
      redirect_to @project, notice: "Project created"
    else
      load_contributors
      render :new, status: :unprocessable_entity
    end
  end

  def edit; end

  def update
    if @project.update(project_params)
      upsert_contributors
      redirect_to @project, notice: "Project updated"
    else
      load_contributors
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
    params.require(:project).permit(:code, :date, :address, :description, :job_value)
  end

  def load_contributors
    @contributor_types = ContributorType.order(:name)
    @contributors_by_type = Contributor.includes(:contributor_type).order(:company_name)
                                       .group_by { |c| c.contributor_type&.id }
  end

  # ✅ THIS is the “save dropdown into DB” wiring
  def upsert_contributors
    incoming = params.fetch(:contributors, {}) # {"<type_id>" => "<contributor_id>", ...}

    incoming.each do |type_id, contributor_id|
      type = ContributorType.find(type_id)
      role = type.name # store as your role string, consistent with existing schema

      existing = @project.project_contributors.find_by(role: role)

      if contributor_id.blank?
        existing&.destroy
        next
      end

      if existing
        existing.update!(contributor_id: contributor_id)
      else
        @project.project_contributors.create!(contributor_id: contributor_id, role: role)
      end
    end
  end
end