class AddProjectDurationAndStartDate < ActiveRecord::Migration[8.1]
  def change
    add_column :projects, :duration_days, :integer
    add_column :projects, :start_date, :date
    add_column :projects, :schedule_from, :string, null: false, default: "due_date"
    add_check_constraint :projects, "duration_days IS NULL OR duration_days > 0", name: "projects_positive_duration"
    add_check_constraint :projects, "schedule_from IN ('due_date', 'start_date')", name: "projects_schedule_from"
  end
end
