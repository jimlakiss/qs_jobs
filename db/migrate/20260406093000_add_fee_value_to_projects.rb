class AddFeeValueToProjects < ActiveRecord::Migration[8.1]
  def change
    add_column :projects, :fee_value, :decimal
  end
end
