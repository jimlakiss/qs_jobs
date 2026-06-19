# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.1].define(version: 2026_06_19_005000) do
  # These are extensions that must be enabled in order to support this database
  enable_extension "pg_catalog.plpgsql"

  create_table "active_storage_attachments", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.datetime "created_at", null: false
    t.string "name", null: false
    t.bigint "record_id", null: false
    t.string "record_type", null: false
    t.index ["blob_id"], name: "index_active_storage_attachments_on_blob_id"
    t.index ["record_type", "record_id", "name", "blob_id"], name: "index_active_storage_attachments_uniqueness", unique: true
  end

  create_table "active_storage_blobs", force: :cascade do |t|
    t.bigint "byte_size", null: false
    t.string "checksum"
    t.string "content_type"
    t.datetime "created_at", null: false
    t.string "filename", null: false
    t.string "key", null: false
    t.text "metadata"
    t.string "service_name", null: false
    t.index ["key"], name: "index_active_storage_blobs_on_key", unique: true
  end

  create_table "active_storage_variant_records", force: :cascade do |t|
    t.bigint "blob_id", null: false
    t.string "variation_digest", null: false
    t.index ["blob_id", "variation_digest"], name: "index_active_storage_variant_records_uniqueness", unique: true
  end

  create_table "contributor_type_assignments", force: :cascade do |t|
    t.bigint "contributor_id", null: false
    t.bigint "contributor_type_id", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["contributor_id", "contributor_type_id"], name: "index_contributor_type_assignments_uniqueness", unique: true
    t.index ["contributor_id"], name: "index_contributor_type_assignments_on_contributor_id"
    t.index ["contributor_type_id"], name: "index_contributor_type_assignments_on_contributor_type_id"
  end

  create_table "contributor_types", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "name"
    t.datetime "updated_at", null: false
  end

  create_table "contributors", force: :cascade do |t|
    t.text "address"
    t.string "company_name"
    t.datetime "created_at", null: false
    t.string "email"
    t.string "key_contact"
    t.text "notes"
    t.string "phone_number"
    t.datetime "updated_at", null: false
    t.string "url"
  end

  create_table "document_extractions", force: :cascade do |t|
    t.bigint "active_storage_attachment_id", null: false
    t.datetime "created_at", null: false
    t.jsonb "document_details", default: {}, null: false
    t.datetime "extracted_at"
    t.jsonb "measurements", default: {}, null: false
    t.bigint "project_id", null: false
    t.jsonb "regions", default: {}, null: false
    t.jsonb "sheets", default: [], null: false
    t.string "source_filename"
    t.jsonb "staging_data", default: [], null: false
    t.datetime "updated_at", null: false
    t.index ["active_storage_attachment_id"], name: "index_document_extractions_on_active_storage_attachment_id"
    t.index ["project_id", "active_storage_attachment_id"], name: "index_document_extractions_on_project_and_attachment", unique: true
    t.index ["project_id"], name: "index_document_extractions_on_project_id"
  end

  create_table "project_contributors", force: :cascade do |t|
    t.bigint "contributor_id"
    t.datetime "created_at", null: false
    t.bigint "project_id"
    t.string "role"
    t.datetime "updated_at", null: false
    t.index ["contributor_id"], name: "index_project_contributors_on_contributor_id"
    t.index ["project_id", "role"], name: "index_project_contributors_on_project_id_and_role", unique: true
    t.index ["project_id"], name: "index_project_contributors_on_project_id"
  end

  create_table "project_documents", force: :cascade do |t|
    t.bigint "active_storage_attachment_id", null: false
    t.string "category", default: "imported", null: false
    t.datetime "created_at", null: false
    t.bigint "document_extraction_id"
    t.string "export_kind"
    t.bigint "generated_from_attachment_id"
    t.text "notes"
    t.bigint "project_id", null: false
    t.datetime "received_at"
    t.string "received_from"
    t.string "source"
    t.datetime "updated_at", null: false
    t.index ["active_storage_attachment_id"], name: "index_project_documents_on_active_storage_attachment_id", unique: true
    t.index ["document_extraction_id"], name: "index_project_documents_on_document_extraction_id"
    t.index ["project_id", "category"], name: "index_project_documents_on_project_id_and_category"
    t.index ["project_id"], name: "index_project_documents_on_project_id"
    t.index ["received_at"], name: "index_project_documents_on_received_at"
  end

  create_table "projects", force: :cascade do |t|
    t.text "address"
    t.string "code"
    t.datetime "created_at", null: false
    t.date "date"
    t.text "description"
    t.decimal "fee_value"
    t.decimal "job_value"
    t.datetime "updated_at", null: false
    t.index "lower(btrim((code)::text))", name: "index_projects_on_normalized_code_unique", unique: true
  end

  create_table "users", force: :cascade do |t|
    t.datetime "created_at", null: false
    t.string "email", default: "", null: false
    t.string "encrypted_password", default: "", null: false
    t.datetime "remember_created_at"
    t.datetime "reset_password_sent_at"
    t.string "reset_password_token"
    t.datetime "updated_at", null: false
    t.index ["email"], name: "index_users_on_email", unique: true
    t.index ["reset_password_token"], name: "index_users_on_reset_password_token", unique: true
  end

  add_foreign_key "active_storage_attachments", "active_storage_blobs", column: "blob_id"
  add_foreign_key "active_storage_variant_records", "active_storage_blobs", column: "blob_id"
  add_foreign_key "contributor_type_assignments", "contributor_types"
  add_foreign_key "contributor_type_assignments", "contributors"
  add_foreign_key "document_extractions", "projects"
  add_foreign_key "project_contributors", "contributors"
  add_foreign_key "project_contributors", "projects"
  add_foreign_key "project_documents", "document_extractions"
  add_foreign_key "project_documents", "projects"
end
