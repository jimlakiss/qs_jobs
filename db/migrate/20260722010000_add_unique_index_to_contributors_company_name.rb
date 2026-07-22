class AddUniqueIndexToContributorsCompanyName < ActiveRecord::Migration[8.1]
  def up
    execute <<~SQL.squish
      UPDATE contributors
      SET company_name = NULLIF(btrim(company_name), '')
      WHERE company_name IS NOT NULL
    SQL

    add_index :contributors,
      "lower(btrim(company_name))",
      unique: true,
      name: "index_contributors_on_normalized_company_name_unique"
  end

  def down
    remove_index :contributors, name: "index_contributors_on_normalized_company_name_unique"
  end
end
