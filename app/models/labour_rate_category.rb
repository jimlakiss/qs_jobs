class LabourRateCategory < ApplicationRecord
  has_many :labour_rates, foreign_key: :category_key, primary_key: :key, dependent: :restrict_with_error
  attr_readonly :key
  before_validation :normalize_fields
  before_validation :assign_key, on: :create

  validates :key, presence: true, uniqueness: true
  validates :code, presence: true, length: { maximum: 20 },
    format: { with: /\A[A-Z0-9.\-]+\z/, message: "can only contain letters, numbers, dots and hyphens" },
    uniqueness: { case_sensitive: false }
  validates :name, :section, presence: true, length: { maximum: 120 }
  validates :position, numericality: { only_integer: true, greater_than_or_equal_to: 0 }

  scope :active, -> { where(active: true) }
  scope :ordered, -> { order(:position, :code) }

  def catalogue_entry
    { key: key, code: code, name: name, subheading: subheading }
  end

  private

  def normalize_fields
    self.code = code.to_s.strip.upcase
    self.name = name.to_s.strip
    self.section = section.to_s.strip
  end

  def assign_key
    self.key ||= SecureRandom.uuid
  end
end
