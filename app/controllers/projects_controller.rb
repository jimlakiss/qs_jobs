class ProjectsController < ApplicationController
  def index
    @projects = Project
      .includes(project_contributors: :contributor)
      .order(created_at: :desc)
  end

  def show
    @project = Project.find(params[:id])
  end
end