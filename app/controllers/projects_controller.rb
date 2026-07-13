class ProjectsController < ApplicationController
  PROJECT_SORT_OPTIONS = {
    "code_asc" => {
      label: "Code: lowest to highest",
      order: Arel.sql("LOWER(projects.code) ASC, projects.code ASC, projects.id ASC")
    },
    "code_desc" => {
      label: "Code: highest to lowest",
      order: Arel.sql("LOWER(projects.code) DESC, projects.code DESC, projects.id DESC")
    }
  }.freeze

  before_action :set_project, only: [:show, :edit, :update, :destroy, :confirm_destroy]
  before_action :load_contributors, only: [:new, :edit]

  def index
    @query = params[:q].to_s.strip
    @sort = params[:sort].presence_in(PROJECT_SORT_OPTIONS.keys) || "code_asc"
    @project_sort_options = PROJECT_SORT_OPTIONS.map { |value, config| [config.fetch(:label), value] }
    @projects = Project.includes(project_contributors: :contributor)
    @projects = @projects.where(
      "projects.code ILIKE :q OR projects.address ILIKE :q OR projects.description ILIKE :q",
      q: "%#{@query}%"
    ) if @query.present?
    @projects = @projects.order(PROJECT_SORT_OPTIONS.fetch(@sort).fetch(:order))
    @total_job_value = @projects.sum(:job_value)
    @total_fee_value = @projects.sum(:fee_value)
    @matching_contributors = Contributor.none
    @matching_contributor_types = ContributorType.none

    if @query.present?
      @matching_contributors = Contributor.includes(:contributor_types).where(
        "company_name ILIKE :q OR key_contact ILIKE :q OR email ILIKE :q",
        q: "%#{@query}%"
      ).order(:company_name).limit(10)

      @matching_contributor_types = ContributorType.where(
        "name ILIKE :q",
        q: "%#{@query}%"
      ).order(:name).limit(10)
    end
  end

  def show
    @document_extractions = @project.document_extractions.order(extracted_at: :desc, updated_at: :desc)
    @document_extractions_by_attachment_id = @document_extractions.index_by(&:active_storage_attachment_id)
    @project_documents_by_attachment_id = @project.project_documents.index_by(&:active_storage_attachment_id)
    @imported_documents = @project.documents.select do |document|
      @project_documents_by_attachment_id[document.id]&.category != "extracted_document"
    end
    @extracted_documents = @project.documents.select do |document|
      @project_documents_by_attachment_id[document.id]&.category == "extracted_document"
    end
  end

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

  def confirm_destroy
    @assigned_contributors = @project.project_contributors.includes(:contributor)
  end

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
    params.require(:project).permit(:code, :date, :address, :description, :job_value, :fee_value)
  end

  def load_contributors
    @contributor_types = ContributorType.order(:name)
    @contributors_by_type = Hash.new { |hash, key| hash[key] = [] }

    Contributor.includes(:contributor_types)
               .order(Arel.sql("LOWER(company_name), company_name"))
               .each do |contributor|
      contributor.contributor_types.each do |type|
        @contributors_by_type[type.id] << contributor
      end
    end
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
