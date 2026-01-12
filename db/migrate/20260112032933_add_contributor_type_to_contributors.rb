class AddContributorTypeToContributors < ActiveRecord::Migration[8.1]
  def change
    add_column :contributors, :contributor_type, :string
  end
end
