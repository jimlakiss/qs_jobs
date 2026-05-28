class CreateContributorTypeAssignments < ActiveRecord::Migration[8.1]
  class MigrationContributor < ApplicationRecord
    self.table_name = "contributors"
  end

  class MigrationContributorType < ApplicationRecord
    self.table_name = "contributor_types"
  end

  def up
    create_table :contributor_type_assignments do |t|
      t.references :contributor, null: false, foreign_key: true
      t.references :contributor_type, null: false, foreign_key: true

      t.timestamps
    end

    add_index :contributor_type_assignments,
              [:contributor_id, :contributor_type_id],
              unique: true,
              name: "index_contributor_type_assignments_uniqueness"

    say_with_time "Migrating contributor type assignments" do
      MigrationContributor.reset_column_information

      MigrationContributor.find_each do |contributor|
        contributor_type_id = contributor[:contributor_type_id]

        if contributor_type_id.blank? && contributor[:contributor_type].present?
          contributor_type_id = MigrationContributorType.find_or_create_by!(
            name: contributor[:contributor_type].strip
          ).id
        end

        next if contributor_type_id.blank?

        execute <<~SQL.squish
          INSERT INTO contributor_type_assignments
            (contributor_id, contributor_type_id, created_at, updated_at)
          VALUES
            (#{contributor.id}, #{contributor_type_id}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
          ON CONFLICT (contributor_id, contributor_type_id) DO NOTHING
        SQL
      end
    end

    remove_reference :contributors, :contributor_type, foreign_key: true
    remove_column :contributors, :contributor_type, :string
  end

  def down
    add_column :contributors, :contributor_type, :string
    add_reference :contributors, :contributor_type, foreign_key: true

    say_with_time "Restoring primary contributor types" do
      execute <<~SQL.squish
        UPDATE contributors
        SET contributor_type_id = ranked.contributor_type_id,
            contributor_type = contributor_types.name
        FROM (
          SELECT DISTINCT ON (contributor_id)
            contributor_id,
            contributor_type_id
          FROM contributor_type_assignments
          ORDER BY contributor_id, created_at ASC, id ASC
        ) ranked
        INNER JOIN contributor_types
          ON contributor_types.id = ranked.contributor_type_id
        WHERE contributors.id = ranked.contributor_id
      SQL
    end

    remove_index :contributor_type_assignments, name: "index_contributor_type_assignments_uniqueness"
    drop_table :contributor_type_assignments
  end
end
