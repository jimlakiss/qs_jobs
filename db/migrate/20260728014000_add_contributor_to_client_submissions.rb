class AddContributorToClientSubmissions < ActiveRecord::Migration[8.1]
  def change
    add_reference :client_submissions, :contributor, foreign_key: true

    reversible do |dir|
      dir.up do
        execute <<~SQL.squish
          UPDATE client_submissions
          SET contributor_id = users.contributor_id
          FROM users
          WHERE client_submissions.submitted_by_id = users.id
        SQL
      end
    end

    change_column_null :client_submissions, :contributor_id, false
  end
end
