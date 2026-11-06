module ContributorsHelper
  def contributor_website_url(contributor)
    uri = URI.parse(contributor.url.to_s)
    uri.to_s if uri.is_a?(URI::HTTP) && uri.host.present?
  rescue URI::InvalidURIError
    nil
  end
end
