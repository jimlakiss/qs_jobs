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

  def grouped_documents(documents, metadata_by_attachment_id)
    documents.group_by do |document|
      metadata_by_attachment_id[document.id]&.document_group&.name.presence || "Ungrouped Documents"
    end.sort_by do |group_name, _group_documents|
      [group_name == "Ungrouped Documents" ? 1 : 0, group_name.downcase]
    end
  end
end
