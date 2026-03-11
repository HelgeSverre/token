# Nim Syntax Highlighting Test
# A concurrent web crawler with metaprogramming and effect system.

import std/[
  asyncdispatch, httpclient, uri, strutils, strformat,
  tables, sets, times, os, json, re, sequtils, sugar,
  locks, atomics, parseopt, logging, hashes
]

const
  Version = "1.0.0"
  MaxDepth = 5
  MaxConcurrent = 10
  DefaultTimeout = 30_000
  UserAgent = fmt"NimCrawler/{Version}"

type
  PageStatus = enum
    psSuccess = "success"
    psRedirect = "redirect"
    psError = "error"
    psTimeout = "timeout"

  CrawlResult = object
    url: string
    status: PageStatus
    statusCode: int
    title: string
    links: seq[string]
    fetchTime: Duration
    depth: int
    error: string

  CrawlerConfig = object
    maxDepth: int
    maxPages: int
    concurrency: int
    timeout: int
    allowedDomains: seq[string]
    excludePatterns: seq[Regex]
    respectRobotsTxt: bool

  CrawlerStats = object
    pagesVisited: int
    pagesSkipped: int
    errorsCount: int
    totalBytes: int64
    startTime: Time
    bytesPerSecond: float

# ============================================================
# Utility templates and macros
# ============================================================

template benchmark(label: string, body: untyped): untyped =
  let t0 = cpuTime()
  body
  let elapsed = cpuTime() - t0
  debug fmt"{label}: {elapsed * 1000:.2f}ms"

macro generateAccessors(T: typedesc, fields: varargs[untyped]): untyped =
  ## Auto-generate getter/setter procs for fields
  result = newStmtList()
  for field in fields:
    let
      fieldStr = $field
      getterName = ident("get" & capitalizeAscii(fieldStr))
      setterName = ident("set" & capitalizeAscii(fieldStr))

    result.add quote do:
      proc `getterName`(self: `T`): auto = self.`field`
      proc `setterName`(self: var `T`, val: auto) = self.`field` = val

# ============================================================
# URL processing
# ============================================================

proc normalizeUrl(base, relative: string): string =
  ## Resolve a relative URL against a base URL
  if relative.startsWith("http://") or relative.startsWith("https://"):
    return relative

  let baseUri = parseUri(base)

  if relative.startsWith("//"):
    return baseUri.scheme & ":" & relative
  elif relative.startsWith("/"):
    return fmt"{baseUri.scheme}://{baseUri.hostname}{relative}"
  else:
    let basePath = baseUri.path.rsplit("/", maxsplit = 1)[0]
    return fmt"{baseUri.scheme}://{baseUri.hostname}{basePath}/{relative}"

proc extractDomain(url: string): string =
  parseUri(url).hostname

proc isAllowedUrl(url: string, config: CrawlerConfig): bool =
  let domain = extractDomain(url)

  if config.allowedDomains.len > 0 and domain notin config.allowedDomains:
    return false

  for pattern in config.excludePatterns:
    if url.match(pattern):
      return false

  return true

# ============================================================
# HTML parsing (simplified)
# ============================================================

proc extractLinks(html, baseUrl: string): seq[string] =
  ## Extract all href links from HTML content
  var links: seq[string] = @[]
  let pattern = re"""href\s*=\s*["']([^"']+)["']"""

  for match in html.findAll(pattern):
    let href = html[match.first..match.last]
      .replace(re"""href\s*=\s*["']""", "")
      .strip(chars = {'"', '\''})

    if href.len > 0 and not href.startsWith("#") and
       not href.startsWith("javascript:") and
       not href.startsWith("mailto:"):
      links.add(normalizeUrl(baseUrl, href))

  links.deduplicate()

proc extractTitle(html: string): string =
  let match = html.find(re"<title[^>]*>(.*?)</title>")
  if match.isSome:
    html[match.get.first..match.get.last]
      .replace(re"</?title[^>]*>", "")
      .strip()
  else:
    ""

# ============================================================
# Crawler implementation
# ============================================================

type
  Crawler = ref object
    config: CrawlerConfig
    visited: HashSet[string]
    results: seq[CrawlResult]
    stats: CrawlerStats
    client: AsyncHttpClient
    semaphore: int  # Simple concurrency limiter
    lock: Lock

proc newCrawler(config: CrawlerConfig): Crawler =
  result = Crawler(
    config: config,
    visited: initHashSet[string](),
    results: @[],
    stats: CrawlerStats(startTime: getTime()),
    client: newAsyncHttpClient(
      userAgent = UserAgent,
      maxRedirects = 5,
    ),
    semaphore: config.concurrency,
  )
  initLock(result.lock)

proc isVisited(crawler: Crawler, url: string): bool =
  withLock(crawler.lock):
    url in crawler.visited

proc markVisited(crawler: Crawler, url: string) =
  withLock(crawler.lock):
    crawler.visited.incl(url)

proc addResult(crawler: Crawler, result: CrawlResult) =
  withLock(crawler.lock):
    crawler.results.add(result)
    case result.status
    of psSuccess, psRedirect:
      inc crawler.stats.pagesVisited
    of psError, psTimeout:
      inc crawler.stats.errorsCount

proc fetchPage(crawler: Crawler, url: string, depth: int): Future[CrawlResult] {.async.} =
  let startTime = getTime()

  try:
    let response = await crawler.client.get(url)
    let body = await response.body
    let elapsed = getTime() - startTime

    crawler.stats.totalBytes += body.len.int64

    let links = if depth < crawler.config.maxDepth:
                  extractLinks(body, url)
                    .filterIt(isAllowedUrl(it, crawler.config))
                else: @[]

    result = CrawlResult(
      url: url,
      status: if response.code.int in 200..299: psSuccess
              elif response.code.int in 300..399: psRedirect
              else: psError,
      statusCode: response.code.int,
      title: extractTitle(body),
      links: links,
      fetchTime: elapsed,
      depth: depth,
    )

  except TimeoutError:
    result = CrawlResult(
      url: url, status: psTimeout, depth: depth,
      fetchTime: getTime() - startTime,
      error: "Request timed out",
    )

  except CatchableError as e:
    result = CrawlResult(
      url: url, status: psError, depth: depth,
      fetchTime: getTime() - startTime,
      error: e.msg,
    )

proc crawl(crawler: Crawler, startUrl: string) {.async.} =
  ## Main crawl loop using BFS
  var queue: seq[(string, int)] = @[(startUrl, 0)]

  while queue.len > 0 and
        crawler.stats.pagesVisited < crawler.config.maxPages:

    # Take a batch
    let batchSize = min(queue.len, crawler.config.concurrency)
    let batch = queue[0..<batchSize]
    queue = queue[batchSize..^1]

    var futures: seq[Future[CrawlResult]] = @[]

    for (url, depth) in batch:
      if crawler.isVisited(url):
        inc crawler.stats.pagesSkipped
        continue

      crawler.markVisited(url)
      futures.add(crawler.fetchPage(url, depth))

    # Await all in parallel
    for future in futures:
      let pageResult = await future
      crawler.addResult(pageResult)

      info fmt"[{pageResult.status}] {pageResult.url} ({pageResult.fetchTime})"

      if pageResult.status == psSuccess:
        for link in pageResult.links:
          if not crawler.isVisited(link) and
             pageResult.depth + 1 <= crawler.config.maxDepth:
            queue.add((link, pageResult.depth + 1))

# ============================================================
# Report generation
# ============================================================

proc toJson(stats: CrawlerStats): JsonNode =
  let elapsed = (getTime() - stats.startTime).inSeconds.float
  %*{
    "pages_visited": stats.pagesVisited,
    "pages_skipped": stats.pagesSkipped,
    "errors": stats.errorsCount,
    "total_bytes": stats.totalBytes,
    "elapsed_seconds": elapsed,
    "pages_per_second": if elapsed > 0: stats.pagesVisited.float / elapsed
                        else: 0.0,
  }

proc printReport(crawler: Crawler) =
  let elapsed = getTime() - crawler.stats.startTime

  echo fmt"""
╔══════════════════════════════════════╗
║          Crawl Report                ║
╠══════════════════════════════════════╣
║ Pages visited:  {crawler.stats.pagesVisited:>6}             ║
║ Pages skipped:  {crawler.stats.pagesSkipped:>6}             ║
║ Errors:         {crawler.stats.errorsCount:>6}             ║
║ Total data:     {crawler.stats.totalBytes div 1024:>6} KB          ║
║ Duration:       {elapsed.inSeconds:>6}s            ║
╚══════════════════════════════════════╝"""

  # Top pages by link count
  echo "\nTop 10 most-linked pages:"
  var linkCounts = initCountTable[string]()
  for r in crawler.results:
    for link in r.links:
      linkCounts.inc(link)

  for url, count in linkCounts.pairs.toSeq
      .sortedByIt(-it[1])[0..<min(10, linkCounts.len)]:
    echo fmt"  {count:>4}x  {url}"

# ============================================================
# Main
# ============================================================

proc main() =
  var config = CrawlerConfig(
    maxDepth: 3,
    maxPages: 100,
    concurrency: MaxConcurrent,
    timeout: DefaultTimeout,
    allowedDomains: @[],
    excludePatterns: @[
      re"\.(jpg|jpeg|png|gif|svg|pdf|zip)$",
      re"/(login|logout|signup)",
    ],
    respectRobotsTxt: true,
  )

  # Parse command line
  for kind, key, val in getopt():
    case kind
    of cmdArgument:
      discard
    of cmdLongOption, cmdShortOption:
      case key
      of "depth", "d": config.maxDepth = parseInt(val)
      of "max-pages", "m": config.maxPages = parseInt(val)
      of "concurrency", "c": config.concurrency = parseInt(val)
      of "help", "h":
        echo fmt"NimCrawler v{Version}"
        echo "Usage: crawler [options] <url>"
        quit(0)
      else: discard
    of cmdEnd: discard

  let startUrl = commandLineParams()[^1]
  echo fmt"Starting crawl of {startUrl} (depth={config.maxDepth}, max={config.maxPages})"

  let crawler = newCrawler(config)

  benchmark "Total crawl time":
    waitFor crawler.crawl(startUrl)

  crawler.printReport()

when isMainModule:
  main()
