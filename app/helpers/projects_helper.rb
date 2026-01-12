module ProjectsHelper
  def contributor_for(project, role)
    project.project_contributors
           .find { |pc| pc.role == role }
           &.contributor
           &.company_name
  end
end