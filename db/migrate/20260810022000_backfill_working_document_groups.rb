class BackfillWorkingDocumentGroups < ActiveRecord::Migration[8.1]
  def up
    execute <<~SQL.squish
      INSERT INTO document_groups (
        groupable_type,
        groupable_id,
        name,
        category,
        created_at,
        updated_at
      )
      SELECT DISTINCT
        source_groups.groupable_type,
        source_groups.groupable_id,
        source_groups.name,
        'working',
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
      FROM project_documents
      INNER JOIN document_groups source_groups
        ON source_groups.id = project_documents.document_group_id
      WHERE project_documents.category = 'extracted_document'
        AND source_groups.category <> 'working'
        AND NOT EXISTS (
          SELECT 1
          FROM document_groups existing_working_groups
          WHERE existing_working_groups.groupable_type = source_groups.groupable_type
            AND existing_working_groups.groupable_id = source_groups.groupable_id
            AND existing_working_groups.category = 'working'
            AND LOWER(BTRIM(existing_working_groups.name)) = LOWER(BTRIM(source_groups.name))
        )
    SQL

    execute <<~SQL.squish
      UPDATE project_documents
      SET document_group_id = working_groups.id
      FROM document_groups source_groups
      INNER JOIN document_groups working_groups
        ON working_groups.groupable_type = source_groups.groupable_type
        AND working_groups.groupable_id = source_groups.groupable_id
        AND working_groups.category = 'working'
        AND LOWER(BTRIM(working_groups.name)) = LOWER(BTRIM(source_groups.name))
      WHERE project_documents.document_group_id = source_groups.id
        AND project_documents.category = 'extracted_document'
        AND source_groups.category <> 'working'
    SQL
  end

  def down
    raise ActiveRecord::IrreversibleMigration
  end
end
