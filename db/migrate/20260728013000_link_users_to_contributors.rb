class LinkUsersToContributors < ActiveRecord::Migration[8.1]
  def change
    add_reference :users, :contributor, foreign_key: true
    add_column :contributors, :project_upload_access, :boolean, null: false, default: false

    add_index :users,
      :contributor_id,
      unique: true,
      where: "contributor_id IS NOT NULL",
      name: "index_users_on_unique_contributor_portal_access"
  end
end
