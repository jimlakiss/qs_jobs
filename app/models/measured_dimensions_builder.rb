class MeasuredDimensionsBuilder
  POINTS_PER_INCH = 72.0
  METERS_PER_INCH = 0.0254

  def initialize(project)
    @project = project
    @attachments_by_id = project.documents.attachments.index_by(&:id)
  end

  def rows
    @project.document_viewer_states.includes(:document_attachment).flat_map do |viewer_state|
      rows_for_state(viewer_state)
    end.sort_by do |row|
      [
        row[:group].downcase,
        row[:sheet_id].to_s.downcase,
        row[:page].to_i,
        row[:name].to_s.downcase,
        row[:type].to_s
      ]
    end
  end

  private

  def rows_for_state(viewer_state)
    data = viewer_state.data || {}
    measurements_by_page = hash_at(data, "measurementsByPage")
    scale_zones_by_page = hash_at(data, "scaleZonesByPage")
    page_dims_by_page = hash_at(data, "pageBaseDimsByPage")
    sheet_details_by_page = hash_at(data, "sheetDetailsByPage")
    staging_by_page = staging_rows_by_page(data.dig("staging", "stagingData"))
    attachment = @attachments_by_id[viewer_state.active_storage_attachment_id] || viewer_state.document_attachment

    measurements_by_page.flat_map do |page_key, measurements|
      page = page_key.to_i
      Array(measurements).filter_map do |measurement|
        next unless measurement.is_a?(Hash)

        dims = page_dims_by_page[page_key] || page_dims_by_page[page.to_s] || page_dims_by_page[page]
        zones = Array(scale_zones_by_page[page_key] || scale_zones_by_page[page.to_s] || scale_zones_by_page[page])
        values = measurement_values(measurement, dims, zones)

        {
          attachment_id: attachment&.id,
          measurement_id: measurement["id"],
          document_filename: attachment&.filename&.to_s || "Document #{viewer_state.active_storage_attachment_id}",
          page: page,
          sheet_id: sheet_id_for_page(page, sheet_details_by_page, staging_by_page),
          group: measurement["label"].to_s.strip.presence || "Ungrouped Measurements",
          name: measurement["name"].to_s.strip.presence || measurement["dimensionName"].to_s.strip.presence,
          type: measurement["type"].to_s,
          scale_label: values[:scale_label],
          area_m2: values[:area_m2],
          perimeter_m: values[:perimeter_m],
          length_m: values[:length_m],
          count: values[:count],
          saved_at: viewer_state.saved_at
        }
      end
    end
  end

  def hash_at(data, key)
    value = data[key]
    value.is_a?(Hash) ? value : {}
  end

  def staging_rows_by_page(staging_data)
    Array(staging_data).each_with_object({}) do |row, by_page|
      next unless row.is_a?(Hash)

      page = row["page"].to_i
      by_page[page] = row if page.positive?
    end
  end

  def sheet_id_for_page(page, sheet_details_by_page, staging_by_page)
    details = sheet_details_by_page[page.to_s] || sheet_details_by_page[page] || {}
    details["sheet_id"].presence || staging_by_page[page]&.dig("sheet_id").presence || "Page #{page}"
  end

  def measurement_values(measurement, dims, zones)
    points = normalized_points(measurement["points"])
    type = measurement["type"].to_s
    return { count: points.size } if type == "count"

    dims = normalize_dims(dims)
    zone = zone_for_measurement(type, points, zones)
    return {} unless dims && zone

    case type
    when "linear"
      { length_m: linear_meters(points, dims, zone), scale_label: zone["label"] }
    when "area"
      {
        area_m2: area_square_meters(points, dims, zone),
        perimeter_m: perimeter_meters(points, dims, zone),
        scale_label: zone["label"]
      }
    else
      {}
    end
  end

  def normalized_points(points)
    Array(points).filter_map do |point|
      next unless point.is_a?(Hash)

      x = point["x"] || point[:x]
      y = point["y"] || point[:y]
      next unless x && y

      { "x" => x.to_f, "y" => y.to_f }
    end
  end

  def normalize_dims(dims)
    return unless dims.is_a?(Hash)

    width = dims["width"] || dims[:width]
    height = dims["height"] || dims[:height]
    return unless width && height

    { "width" => width.to_f, "height" => height.to_f }
  end

  def zone_for_measurement(type, points, zones)
    return if points.empty?

    target = if type == "area" && points.length >= 3
      {
        "x" => points.sum { |point| point["x"] } / points.length,
        "y" => points.sum { |point| point["y"] } / points.length
      }
    else
      points.first
    end

    zones.reverse.find do |zone|
      point_in_poly?(target["x"], target["y"], normalized_points(zone["vertices"]))
    end
  end

  def point_in_poly?(x, y, vertices)
    return false if vertices.length < 3

    inside = false
    j = vertices.length - 1
    vertices.each_with_index do |vertex, i|
      previous = vertices[j]
      if ((vertex["y"] > y) != (previous["y"] > y)) &&
          x < (previous["x"] - vertex["x"]) * (y - vertex["y"]) / (previous["y"] - vertex["y"]) + vertex["x"]
        inside = !inside
      end
      j = i
    end
    inside
  end

  def linear_meters(points, dims, zone)
    points.each_cons(2).sum do |from, to|
      norm_to_meters(from["x"], from["y"], to["x"], to["y"], dims, zone)
    end
  end

  def perimeter_meters(points, dims, zone)
    points.each_with_index.sum do |point, index|
      target = points[(index + 1) % points.length]
      norm_to_meters(point["x"], point["y"], target["x"], target["y"], dims, zone)
    end
  end

  def norm_to_meters(x1, y1, x2, y2, dims, zone)
    dx = (x2 - x1) * dims["width"] / POINTS_PER_INCH * METERS_PER_INCH
    dy = (y2 - y1) * dims["height"] / POINTS_PER_INCH * METERS_PER_INCH
    Math.sqrt((dx * dx) + (dy * dy)) * zone["mpp"].to_f
  end

  def area_square_meters(points, dims, zone)
    area = points.each_with_index.sum do |point, index|
      target = points[(index + 1) % points.length]
      (point["x"] * dims["width"] * target["y"] * dims["height"]) -
        (target["x"] * dims["width"] * point["y"] * dims["height"])
    end

    paper_m2 = area.abs / 2 / (POINTS_PER_INCH * POINTS_PER_INCH) * (METERS_PER_INCH * METERS_PER_INCH)
    paper_m2 * zone["mpp"].to_f * zone["mpp"].to_f
  end
end
