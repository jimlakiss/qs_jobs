# This file should ensure the existence of records required to run the application in every environment.
# Load a workbook directly with:
#   SEED_PROJECT_DATA_XLSX=/path/to/Project\ Date_1.xlsx
#
# Or load CSV files by exporting the spreadsheet tabs and setting one or more of:
#   SEED_CONTRIBUTOR_TYPES_CSV=/path/to/contributor_types.csv
#   SEED_CONTRIBUTORS_CSV=/path/to/contributors.csv
#   SEED_PROJECTS_CSV=/path/to/projects.csv
#   SEED_PROJECT_CONTRIBUTORS_CSV=/path/to/project_contributors.csv

require_relative "seeds/support/spreadsheet_seed_loader"

# Insert missing defaults without overwriting changes made in admin settings.
JSON.parse(File.read(Rails.root.join("db/seeds/labour_rate_categories.json"))).each do |attributes|
  LabourRateCategory.find_or_create_by!(key: attributes.fetch("key")) do |category|
    category.assign_attributes(attributes.except("key"))
  end
end

# Schema-loaded databases need the same initial workflow as migrated databases.
if ProjectStatus.none?
  ProjectStatus::DEFAULT_NAMES.each_with_index do |name, position|
    ProjectStatus.create!(name: name, position: position)
  end
end

SpreadsheetSeedLoader.run
