class AddArchivedAtToClientSubmissions < ActiveRecord::Migration[8.1]
  def change
    add_column :client_submissions, :archived_at, :datetime
    add_index :client_submissions, :archived_at
  end
end
