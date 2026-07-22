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
end
