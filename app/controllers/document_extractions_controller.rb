class DocumentExtractionsController < ApplicationController
  before_action :set_project

  def destroy
    extraction = @project.document_extractions.find(params[:id])
    extraction.purge_generated_documents
    extraction.destroy

    redirect_to project_path(@project, tab: "extracted-data"), notice: "Extracted data deleted"
  end

  private

  def set_project
    @project = Project.find(params[:project_id])
  end
end
