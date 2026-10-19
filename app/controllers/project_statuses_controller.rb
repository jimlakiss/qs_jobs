class ProjectStatusesController < ApplicationController
  before_action :require_admin!

  def index
    @statuses = ProjectStatus.ordered.to_a
    @project_counts = Project.group(:project_status_id).count
    @new_status = ProjectStatus.new(position: (ProjectStatus.maximum(:position) || -1) + 1)
  end

  def create
    status = ProjectStatus.new(status_params)
    if status.save
      redirect_to project_statuses_path, notice: "Status added"
    else
      redirect_to project_statuses_path, alert: status.errors.full_messages.to_sentence
    end
  end

  def update
    status = ProjectStatus.find(params[:id])
    if status.update(status_params)
      redirect_to project_statuses_path, notice: "Status updated"
    else
      redirect_to project_statuses_path, alert: status.errors.full_messages.to_sentence
    end
  end

  def destroy
    status = ProjectStatus.find(params[:id])
    if status.remove_with_replacement(replacement_id: params[:replacement_status_id])
      redirect_to project_statuses_path, notice: "Status deleted"
    else
      redirect_to project_statuses_path, alert: status.errors.full_messages.to_sentence
    end
  rescue ActiveRecord::RecordNotDestroyed, ActiveRecord::InvalidForeignKey
    redirect_to project_statuses_path, alert: "The status could not be deleted. Refresh and try again."
  end

  private

  def status_params
    params.require(:project_status).permit(:name, :position)
  end
end
