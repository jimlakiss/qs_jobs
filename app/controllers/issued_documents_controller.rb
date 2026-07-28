class IssuedDocumentsController < ApplicationController
  before_action :require_client!
  before_action :set_project_issue, only: [:show]

  def index
    @project_issues = current_user.received_project_issues.includes(:project, :contributor).available_to_clients
  end

  def show; end

  private

  def require_client!
    redirect_to projects_path, alert: "You do not have access to that area" unless client_user?
  end

  def set_project_issue
    @project_issue = current_user.received_project_issues.includes(:project, :contributor).available_to_clients.find(params[:id])
  end
end
