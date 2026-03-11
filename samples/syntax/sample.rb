# Ruby Syntax Highlighting Test
# A small web scraper and data pipeline demonstrating Ruby's expressiveness.

require 'net/http'
require 'json'
require 'uri'
require 'digest'

# Frozen string literal magic comment style
MODULE_VERSION = '2.1.0'.freeze
MAX_RETRIES = 3
TIMEOUT = 30

# Symbol and regex literals
VALID_SCHEMES = %i[http https].freeze
URL_PATTERN = /\Ahttps?:\/\/[\w\-.]+(:\d+)?(\/\S*)?\z/i
EMAIL_RE = /\A[\w+\-.]+@[a-z\d\-]+(\.[a-z\d\-]+)*\.[a-z]+\z/i

# Struct for lightweight data objects
PageResult = Struct.new(:url, :status, :body, :fetched_at, keyword_init: true)

# Module with mixin behavior
module Logging
  def log(level, message)
    timestamp = Time.now.strftime('%Y-%m-%d %H:%M:%S')
    puts "[#{timestamp}] #{level.upcase}: #{message}"
  end

  def debug(msg) = log(:debug, msg)
  def info(msg)  = log(:info, msg)
  def warn(msg)  = log(:warn, msg)
  def error(msg) = log(:error, msg)
end

# Error hierarchy
class ScraperError < StandardError; end
class FetchError < ScraperError; end
class ParseError < ScraperError; end

# Main scraper class
class WebScraper
  include Logging

  attr_reader :base_url, :results, :options

  def initialize(base_url, **options)
    @base_url = base_url
    @results = []
    @options = {
      max_pages: 10,
      delay: 0.5,
      user_agent: "RubyScraper/#{MODULE_VERSION}",
      follow_redirects: true
    }.merge(options)

    validate_url!
  end

  # Fetch a single page with retries
  def fetch(path = '/')
    url = URI.join(base_url, path)
    retries = 0

    begin
      info "Fetching #{url}"
      response = perform_request(url)

      case response
      when Net::HTTPSuccess
        result = PageResult.new(
          url: url.to_s,
          status: response.code.to_i,
          body: response.body,
          fetched_at: Time.now
        )
        @results << result
        result
      when Net::HTTPRedirection
        raise FetchError, "Too many redirects" unless options[:follow_redirects]
        fetch(response['location'])
      else
        raise FetchError, "HTTP #{response.code}: #{response.message}"
      end
    rescue Net::OpenTimeout, Net::ReadTimeout => e
      retries += 1
      if retries <= MAX_RETRIES
        warn "Timeout (attempt #{retries}/#{MAX_RETRIES}): #{e.message}"
        sleep(retries * 0.5)
        retry
      end
      raise FetchError, "Failed after #{MAX_RETRIES} retries: #{e.message}"
    end
  end

  # Fetch multiple paths concurrently-ish
  def fetch_all(paths)
    paths.each_with_object([]) do |path, collected|
      result = fetch(path)
      collected << result if result
      sleep(options[:delay]) if options[:delay] > 0
    rescue ScraperError => e
      error "Skipping #{path}: #{e.message}"
    end
  end

  # Extract links using regex (simplified)
  def extract_links(html)
    html.scan(/href=["']([^"']+)["']/).flatten.uniq.select do |link|
      link.start_with?('http', '/')
    end
  end

  # Pipeline: fetch → extract → transform
  def crawl(start_path = '/', depth: 2)
    visited = Set.new
    queue = [[start_path, 0]]

    until queue.empty? || results.size >= options[:max_pages]
      path, current_depth = queue.shift
      next if visited.include?(path) || current_depth > depth

      visited << path
      result = fetch(path)
      next unless result

      if current_depth < depth
        links = extract_links(result.body)
        links.each { |link| queue << [link, current_depth + 1] }
      end

      yield result if block_given?
    end

    results
  end

  private

  def perform_request(uri)
    http = Net::HTTP.new(uri.host, uri.port)
    http.use_ssl = uri.scheme == 'https'
    http.open_timeout = TIMEOUT
    http.read_timeout = TIMEOUT

    request = Net::HTTP::Get.new(uri)
    request['User-Agent'] = options[:user_agent]

    http.request(request)
  end

  def validate_url!
    raise ArgumentError, "Invalid URL: #{base_url}" unless base_url.match?(URL_PATTERN)
  end
end

# Data transformation with Enumerable methods
class ResultProcessor
  include Logging
  include Enumerable

  def initialize(results)
    @results = results
  end

  def each(&block)
    @results.each(&block)
  end

  # Method chaining with transforms
  def summarize
    group_by { |r| URI.parse(r.url).host }
      .transform_values do |pages|
        {
          count: pages.size,
          avg_size: pages.sum { |p| p.body.bytesize } / pages.size,
          statuses: pages.map(&:status).tally,
          first_fetch: pages.min_by(&:fetched_at).fetched_at,
          last_fetch: pages.max_by(&:fetched_at).fetched_at
        }
      end
  end

  # Heredoc for template
  def to_report
    summary = summarize

    <<~REPORT
      === Scrape Report ===
      Generated: #{Time.now}
      Total pages: #{count}

      #{summary.map { |host, stats| format_host(host, stats) }.join("\n\n")}
    REPORT
  end

  private

  def format_host(host, stats)
    <<~HOST.chomp
      Host: #{host}
        Pages: #{stats[:count]}
        Avg size: #{stats[:avg_size]} bytes
        Statuses: #{stats[:statuses].map { |k, v| "#{k}(#{v})" }.join(', ')}
    HOST
  end
end

# Pattern matching (Ruby 3+)
def classify_response(result)
  case result
  in { status: 200, body: /<!DOCTYPE html>/i }
    :html_page
  in { status: 200, body: /\A\s*[\[{]/ }
    :json_response
  in { status: (300..399) }
    :redirect
  in { status: (400..499) }
    :client_error
  in { status: (500..599) }
    :server_error
  else
    :unknown
  end
end

# Proc and lambda
double = ->(x) { x * 2 }
square = proc { |x| x ** 2 }
transform = method(:classify_response)

# Hash and array literals
config = {
  targets: %w[/about /contact /blog /api/health],
  headers: { 'Accept' => 'text/html', 'Cache-Control' => 'no-cache' },
  ignored_extensions: %w[.jpg .png .gif .css .js],
  rate_limit: 2.5
}

# Conditional execution
if __FILE__ == $PROGRAM_NAME
  scraper = WebScraper.new('https://example.com', max_pages: 5)

  results = scraper.crawl('/') do |page|
    type = classify_response(page)
    puts "  → #{page.url} [#{type}]"
  end

  processor = ResultProcessor.new(results)
  puts processor.to_report
end
