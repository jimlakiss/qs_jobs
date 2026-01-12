class AddContributorTypeRefToContributors < ActiveRecord::Migration[8.1]
  def change
    add_reference :contributors, :contributor_type, null: true, foreign_key: true
  end
end
