class LabourRateSection < ApplicationRecord
  has_many :categories, class_name: "LabourRateCategory", foreign_key: :section, primary_key: :name, dependent: :restrict_with_error

  validates :name, presence: true, uniqueness: true
  validates :position, numericality: { only_integer: true, greater_than_or_equal_to: 0 }
  scope :ordered, -> { order(:position, :name) }
end
