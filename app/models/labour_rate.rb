class LabourRate < ApplicationRecord
  BANDS = %w[low mid high].freeze

  belongs_to :user
  belongs_to :category, class_name: "LabourRateCategory", foreign_key: :category_key, primary_key: :key
  validates :category_key, uniqueness: { scope: :user_id }
  validate :category_is_active, if: -> { new_record? || changed? }
  validates(*BANDS, numericality: { greater_than_or_equal_to: 0, less_than: 100_000_000 }, allow_nil: true)
  validate :bands_in_order

  def populated?
    BANDS.any? { |band| public_send(band).present? }
  end

  def self.client_averages(excluding_user: nil)
    rates = joins(:user, :category).merge(User.client).merge(LabourRateCategory.active)
    rates = rates.where.not(user_id: excluding_user.id) if excluding_user
    rates.group(:category_key)
      .pluck(:category_key, Arel.sql("AVG(low), COUNT(low), AVG(mid), COUNT(mid), AVG(high), COUNT(high)"))
      .to_h do |key, low, low_count, mid, mid_count, high, high_count|
        [ key, {
          "low" => { average: low, count: low_count },
          "mid" => { average: mid, count: mid_count },
          "high" => { average: high, count: high_count }
        } ]
      end
  end

  private

  def category_is_active
    errors.add(:category_key, "is no longer available") unless category&.active?
  end

  def bands_in_order
    return if errors.any?

    values = BANDS.filter_map { |band| public_send(band) }
    errors.add(:base, "Low, Mid and High must be in ascending order") unless values == values.sort
  end
end
