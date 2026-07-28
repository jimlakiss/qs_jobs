class MakeProjectIssueRevisionAndVersionOptional < ActiveRecord::Migration[8.1]
  def change
    change_column_default :project_issues, :revision, from: "X", to: nil
    change_column_default :project_issues, :version, from: "X", to: nil
    change_column_null :project_issues, :revision, true
    change_column_null :project_issues, :version, true
  end
end
