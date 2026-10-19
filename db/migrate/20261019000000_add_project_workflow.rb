class AddProjectWorkflow < ActiveRecord::Migration[8.1]
  def up
    create_table :project_statuses do |t|
      t.string :name, null: false
      t.integer :position, null: false, default: 0
      t.timestamps
    end
    add_index :project_statuses, "lower(name)", unique: true

    names = [
      "Entered - Not Quoted",
      "Quoted - Waiting for Approval",
      "Approved - To Be Completed",
      "Job Being Completed",
      "Done - Not Invoiced"
    ]
    names.each_with_index do |name, position|
      execute "INSERT INTO project_statuses (name, position, created_at, updated_at) VALUES (#{connection.quote(name)}, #{position}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    end
    first_status_id = select_value("SELECT id FROM project_statuses ORDER BY position, id LIMIT 1")
    add_reference :projects, :project_status, foreign_key: true
    add_column :projects, :due_date, :date
    add_column :projects, :archived_at, :datetime
    add_index :projects, :archived_at
    # Keep the existing register in Projects; new jobs start on the current board.
    execute "UPDATE projects SET project_status_id = #{first_status_id}, archived_at = CURRENT_TIMESTAMP"
    change_column_null :projects, :project_status_id, false
  end

  def down
    remove_reference :projects, :project_status, foreign_key: true
    remove_column :projects, :due_date
    remove_column :projects, :archived_at
    drop_table :project_statuses
  end
end
