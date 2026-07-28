module ApplicationHelper
  SYDNEY_TIME_ZONE = "Australia/Sydney"

  def sydney_time(value)
    value&.in_time_zone(SYDNEY_TIME_ZONE)
  end

  def sydney_datetime(value, fallback: "—")
    time = sydney_time(value)
    time ? time.strftime("%d %b %Y %H:%M") : fallback
  end

  def sydney_datetime_local_value(value)
    sydney_time(value)&.strftime("%Y-%m-%dT%H:%M")
  end

  def sortable_table_header(label, sort_key, current_sort:, current_direction:)
    active = current_sort == sort_key.to_s
    next_direction = active && current_direction == "asc" ? "desc" : "asc"
    label_text = active ? "#{label} (#{current_direction})" : label

    link_to label_text,
      url_for(request.query_parameters.merge(sort: sort_key, direction: next_direction)),
      class: "text-decoration-none fw-semibold"
  end
end
