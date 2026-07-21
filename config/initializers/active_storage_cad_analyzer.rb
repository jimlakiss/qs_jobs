# frozen_string_literal: true

require "active_storage/analyzer"

class ActiveStorage::Analyzer::CadAnalyzer < ActiveStorage::Analyzer
  CAD_CONTENT_TYPES = %w[
    application/acad
    application/autocad_dwg
    application/dwg
    application/x-acad
    application/x-autocad
    application/x-dwg
    drawing/x-dwg
    image/vnd.dwg
    image/x-dwg
  ].freeze
  CAD_EXTENSIONS = %w[dwg dxf].freeze

  def self.accept?(blob)
    CAD_CONTENT_TYPES.include?(blob.content_type) || CAD_EXTENSIONS.include?(blob.filename.extension.to_s.downcase)
  end

  def self.analyze_later?
    false
  end

  def metadata
    {}
  end
end

Rails.application.config.active_storage.analyzers.prepend ActiveStorage::Analyzer::CadAnalyzer
