class LabourRateCatalogue
  def self.groups
    categories_by_section = LabourRateCategory.active.ordered.group_by(&:section)
    LabourRateSection.ordered.each_with_object({}) do |section, groups|
      categories = categories_by_section[section.name]
      groups[section.name] = categories.map(&:catalogue_entry) if categories.present?
    end
  end

  def self.entries
    groups.values.flatten
  end

  def self.keys
    LabourRateCategory.active.pluck(:key)
  end

  def self.code_for(key)
    LabourRateCategory.find_by(key: key)&.code
  end
end
