require "date"
require "rexml/document"

class SpreadsheetSeedLoader
  EXCEL_EPOCH = Date.new(1899, 12, 30)

  CONTRIBUTOR_MAPPINGS = [
    { type: "Owner", company: "Client Name" },
    {
      type: "Builder",
      company: "Builder Company Name",
      contact: "Builder Company Contact",
      phone: "Builder Contact Number",
      email: "Builder Contact Email",
      address: "Builder Address"
    },
    {
      type: "Project Architect",
      company: "Project Architect Name",
      contact: "Project Architect Contact",
      phone: "Project Architect Contact",
      email: "Project Architect Email"
    },
    {
      type: "Designer",
      company: "Designer Company Name",
      contact: "Designer Company Contact",
      email: "Designer Contact Email"
    },
    {
      type: "Structural Engineer",
      company: "Structural Engineer Company Name",
      contact: "Structural Engineer Company Contact",
      email: "Structural Engineer Company Email"
    },
    {
      type: "Stormwater Engineer",
      company: "Stormwater Engineer Company Name",
      contact: "Stormwater Engineer Company Contact",
      email: "Stormwater Engineer Company Email"
    },
    { type: "BASIX Consultant", company: "BASIX Consultant" },
    { type: "NATHERS Consultant", company: "NATHERS Consultant" }
  ].freeze

  def self.run
    new.run
  end

  def run
    if ENV["SEED_PROJECT_DATA_XLSX"].present?
      load_project_workbook(ENV["SEED_PROJECT_DATA_XLSX"])
    else
      load_csv_exports
    end
  end

  private

  def load_project_workbook(path)
    rows = workbook_rows(path)
    return if rows.empty?

    headers = rows.first.map { |value| normalize_header(value) }
    seed_contributor_types

    rows.drop(1).each do |values|
      row = headers.each_with_index.with_object({}) do |(header, index), memo|
        next if header.blank?

        memo[header] = values[index].to_s.strip
      end

      next if row.values.all?(&:blank?)

      project = upsert_project(row)
      next unless project

      upsert_project_contributors(project, row)
    end
  end

  def load_csv_exports
    load_contributor_types
    load_contributors
    load_projects
    load_project_contributors
  end

  def upsert_project(row)
    code = row["job id"].presence || row["code"].presence
    return if code.blank?

    Project.find_or_initialize_by(code: code).tap do |project|
      project.assign_attributes(
        date: parse_excel_or_date(row["date"]),
        address: row["address"],
        description: row["description of job"].presence || row["description"],
        job_value: parse_decimal(row["job value (ex gst)"].presence || row["job value"])
      )
      project.save!
    end
  end

  def upsert_project_contributors(project, row)
    CONTRIBUTOR_MAPPINGS.each do |mapping|
      contributor = upsert_contributor_from_mapping(mapping, row)
      next unless contributor

      project.project_contributors.find_or_initialize_by(role: mapping[:type]).tap do |project_contributor|
        project_contributor.contributor = contributor
        project_contributor.save!
      end
    end
  end

  def upsert_contributor_from_mapping(mapping, row)
    company_name = row[normalize_header(mapping[:company])]
    return if company_name.blank?

    contributor = Contributor.find_or_initialize_by(company_name: company_name)
    contributor.assign_attributes(
      contributor_type: ContributorType.find_or_create_by!(name: mapping[:type]),
      key_contact: row[normalize_header(mapping[:contact])],
      address: row[normalize_header(mapping[:address])],
      phone_number: normalize_phone(row[normalize_header(mapping[:phone])]),
      email: row[normalize_header(mapping[:email])]
    )
    contributor.save!
    contributor
  end

  def seed_contributor_types
    CONTRIBUTOR_MAPPINGS.each do |mapping|
      ContributorType.find_or_create_by!(name: mapping[:type])
    end
  end

  def workbook_rows(path)
    shared_strings = extract_shared_strings(path)
    sheet_xml = unzip_file(path, "xl/worksheets/sheet1.xml")
    sheet_doc = REXML::Document.new(sheet_xml)

    REXML::XPath.match(sheet_doc, "//sheetData/row").map do |row|
      values = []

      row.elements.each("c") do |cell|
        column_index = excel_column_to_index(cell.attributes["r"][/[A-Z]+/])
        values[column_index] = read_cell_value(cell, shared_strings)
      end

      values
    end
  end

  def extract_shared_strings(path)
    shared_doc = REXML::Document.new(unzip_file(path, "xl/sharedStrings.xml"))

    REXML::XPath.match(shared_doc, "//si").map do |item|
      REXML::XPath.match(item, ".//t").map { |node| node.text.to_s }.join
    end
  end

  def unzip_file(path, internal_path)
    IO.popen(["unzip", "-p", path, internal_path], &:read)
  end

  def read_cell_value(cell, shared_strings)
    raw = cell.elements["v"]&.text
    return "" if raw.blank?

    case cell.attributes["t"]
    when "s"
      shared_strings[raw.to_i].to_s
    else
      raw
    end
  end

  def excel_column_to_index(column_letters)
    column_letters.each_byte.reduce(0) { |memo, byte| (memo * 26) + byte - 64 } - 1
  end

  def normalize_header(value)
    value.to_s.strip.downcase
  end

  def normalize_phone(value)
    stripped = value.to_s.strip
    return if stripped.blank?

    # Some spreadsheet columns use a phone field as a generic contact field.
    stripped.match?(/\A[\d\s+()\-]+\z/) ? stripped : nil
  end

  def parse_excel_or_date(value)
    return if value.blank?

    return EXCEL_EPOCH + value.to_i if value.to_s.match?(/\A\d+\z/)

    Date.parse(value)
  rescue Date::Error
    nil
  end

  def parse_decimal(value)
    return if value.blank?

    value.to_s.gsub(/[^\d.]/, "").presence
  end

  def load_contributor_types
    each_csv_row(ENV["SEED_CONTRIBUTOR_TYPES_CSV"]) do |row|
      name = first_present(row, "name", "type", "contributor_type")
      next if name.blank?

      ContributorType.find_or_create_by!(name: name.strip)
    end
  end

  def load_contributors
    each_csv_row(ENV["SEED_CONTRIBUTORS_CSV"]) do |row|
      company_name = first_present(row, "company_name", "company", "name")
      next if company_name.blank?

      contributor = Contributor.find_or_initialize_by(company_name: company_name.strip)
      contributor.assign_attributes(
        contributor_type: lookup_contributor_type(first_present(row, "contributor_type", "type")),
        key_contact: first_present(row, "key_contact", "contact"),
        address: first_present(row, "address"),
        phone_number: first_present(row, "phone_number", "phone", "mobile"),
        email: first_present(row, "email"),
        url: first_present(row, "url", "website"),
        notes: first_present(row, "notes")
      )
      contributor.save!
    end
  end

  def load_projects
    each_csv_row(ENV["SEED_PROJECTS_CSV"]) do |row|
      code = first_present(row, "code", "project_code", "job_code")
      next if code.blank?

      project = Project.find_or_initialize_by(code: code.strip)
      project.assign_attributes(
        date: parse_excel_or_date(first_present(row, "date")),
        address: first_present(row, "address"),
        description: first_present(row, "description", "scope"),
        job_value: parse_decimal(first_present(row, "job_value", "value"))
      )
      project.save!
    end
  end

  def load_project_contributors
    each_csv_row(ENV["SEED_PROJECT_CONTRIBUTORS_CSV"]) do |row|
      project_code = first_present(row, "project_code", "code", "job_code")
      role = first_present(row, "role", "contributor_type", "type")
      contributor_name = first_present(row, "company_name", "contributor", "company")

      next if [project_code, role, contributor_name].any?(&:blank?)

      project = Project.find_by!(code: project_code.strip)
      contributor = Contributor.find_by!(company_name: contributor_name.strip)

      project.project_contributors.find_or_initialize_by(role: role.strip).tap do |project_contributor|
        project_contributor.contributor = contributor
        project_contributor.save!
      end
    end
  end

  def lookup_contributor_type(name)
    return if name.blank?

    ContributorType.find_or_create_by!(name: name.strip)
  end

  def each_csv_row(path)
    return if path.blank?

    require "csv"

    CSV.foreach(path, headers: true) do |row|
      yield normalize_csv_row(row.to_h)
    end
  end

  def normalize_csv_row(row)
    row.each_with_object({}) do |(key, value), normalized|
      normalized[key.to_s.strip.downcase] = value.is_a?(String) ? value.strip : value
    end
  end

  def first_present(row, *keys)
    keys.lazy.map { |key| row[key] }.find(&:present?)
  end
end
