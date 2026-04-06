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

SpreadsheetSeedLoader.run
