class RemoveProjectLabourRateSchedules < ActiveRecord::Migration[8.1]
  def up
    drop_table :project_labour_rate_schedules
  end

  def down
    raise ActiveRecord::IrreversibleMigration, "Project rate schedules have been removed"
  end
end
