class LabourRateCatalogue
  def self.groups
    LabourRateCategory.active.ordered.group_by(&:section).transform_values do |categories|
      categories.map(&:catalogue_entry)
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
