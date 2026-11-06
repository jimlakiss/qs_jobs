class AddHourlyRateToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :hourly_rate, :decimal, precision: 10, scale: 2
    add_check_constraint :users, "hourly_rate >= 0", name: "users_non_negative_hourly_rate"
  end
end
