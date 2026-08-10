class RenameExtractedDocumentGroupCategoryToWorking < ActiveRecord::Migration[8.1]
  def up
    execute <<~SQL.squish
      UPDATE document_groups
      SET category = 'working'
      WHERE category = 'extracted'
    SQL
  end

  def down
    execute <<~SQL.squish
      UPDATE document_groups
      SET category = 'extracted'
      WHERE category = 'working'
    SQL
  end
end
