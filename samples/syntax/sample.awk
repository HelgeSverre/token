#!/usr/bin/awk -f

# AWK Syntax Highlighting Test
# A log file analyzer that parses nginx access logs and generates reports.
#
# Usage: awk -f sample.awk access.log
# Input format: Combined Log Format
#   192.168.1.1 - user [10/Oct/2024:13:55:36 -0700] "GET /api/users HTTP/1.1" 200 2326 "https://example.com" "Mozilla/5.0"

BEGIN {
    FS = " "
    OFS = "\t"

    # Configuration
    TOP_N = 10
    SLOW_THRESHOLD = 1.0  # seconds

    # Counters
    total_requests = 0
    total_bytes = 0
    error_count = 0

    # Status code categories
    status_2xx = 0
    status_3xx = 0
    status_4xx = 0
    status_5xx = 0

    # Header
    print "======================================"
    print "  Nginx Access Log Analysis Report"
    print "======================================"
    print ""
}

# Skip empty lines and comments
/^$/ || /^#/ { next }

# Main processing rule - parse each log line
{
    total_requests++

    # Extract fields
    ip = $1
    user = $3

    # Parse timestamp: [10/Oct/2024:13:55:36 -0700]
    match($0, /\[([^\]]+)\]/, ts_arr)
    timestamp = ts_arr[1]

    # Parse request: "GET /api/users HTTP/1.1"
    match($0, /"([A-Z]+) ([^ ]+) HTTP\/[0-9.]+"/, req_arr)
    method = req_arr[1]
    path = req_arr[2]

    # Status code and bytes
    match($0, /" ([0-9]+) ([0-9]+|-) /, resp_arr)
    status = int(resp_arr[1])
    bytes = (resp_arr[2] == "-") ? 0 : int(resp_arr[2])

    total_bytes += bytes

    # Categorize status codes
    if (status >= 200 && status < 300) status_2xx++
    else if (status >= 300 && status < 400) status_3xx++
    else if (status >= 400 && status < 500) {
        status_4xx++
        error_count++
    }
    else if (status >= 500) {
        status_5xx++
        error_count++
    }

    # Track by IP
    ip_count[ip]++
    ip_bytes[ip] += bytes

    # Track by path (normalize query strings)
    clean_path = path
    sub(/\?.*$/, "", clean_path)
    path_count[clean_path]++
    path_status[clean_path, status]++
    path_bytes[clean_path] += bytes

    # Track by method
    method_count[method]++

    # Track by status
    status_count[status]++

    # Track by hour
    split(timestamp, time_parts, ":")
    hour = time_parts[2] ":" "00"
    hourly_count[hour]++

    # Track unique paths per IP (for bot detection)
    ip_paths[ip, clean_path] = 1

    # Track user agents
    match($0, /"([^"]*)"$/, ua_arr)
    ua = ua_arr[1]
    if (ua ~ /[Bb]ot|[Cc]rawler|[Ss]pider|[Ss]craper/) {
        bot_hits[ip]++
    }

    # Track errors for alerting
    if (status >= 500) {
        error_log[error_count] = sprintf("%s %s %s %d", timestamp, method, path, status)
    }
}

# ============================================================
# Helper functions
# ============================================================

function human_bytes(bytes,    units, i) {
    split("B,KB,MB,GB,TB", units, ",")
    i = 1
    while (bytes >= 1024 && i < 5) {
        bytes /= 1024
        i++
    }
    return sprintf("%.1f %s", bytes, units[i])
}

function percentage(part, whole) {
    if (whole == 0) return "0.0%"
    return sprintf("%.1f%%", (part / whole) * 100)
}

function repeat_char(ch, n,    result, i) {
    result = ""
    for (i = 0; i < n; i++) result = result ch
    return result
}

function bar_chart(value, max_val, width,    filled) {
    if (max_val == 0) return repeat_char(" ", width)
    filled = int((value / max_val) * width)
    return repeat_char("█", filled) repeat_char("░", width - filled)
}

# Sort helper: insertion sort on parallel arrays (awk doesn't have sort)
function sort_desc(keys, values, n,    i, j, tk, tv) {
    for (i = 1; i < n; i++) {
        tk = keys[i]
        tv = values[i]
        j = i - 1
        while (j >= 0 && values[j] < tv) {
            keys[j+1] = keys[j]
            values[j+1] = values[j]
            j--
        }
        keys[j+1] = tk
        values[j+1] = tv
    }
}

# ============================================================
# Report generation
# ============================================================

END {
    # ---- Overview ----
    print "── Overview ─────────────────────────"
    printf "  Total requests:  %'d\n", total_requests
    printf "  Total data:      %s\n", human_bytes(total_bytes)
    printf "  Error rate:      %s\n", percentage(error_count, total_requests)
    printf "  Unique IPs:      %d\n", length(ip_count)
    print ""

    # ---- Status Code Breakdown ----
    print "── Status Codes ─────────────────────"
    printf "  2xx (Success):     %6d  %s\n", status_2xx, percentage(status_2xx, total_requests)
    printf "  3xx (Redirect):    %6d  %s\n", status_3xx, percentage(status_3xx, total_requests)
    printf "  4xx (Client Err):  %6d  %s\n", status_4xx, percentage(status_4xx, total_requests)
    printf "  5xx (Server Err):  %6d  %s\n", status_5xx, percentage(status_5xx, total_requests)
    print ""

    # ---- HTTP Methods ----
    print "── HTTP Methods ─────────────────────"
    for (m in method_count) {
        printf "  %-7s  %6d  %s  %s\n",
            m, method_count[m],
            bar_chart(method_count[m], total_requests, 20),
            percentage(method_count[m], total_requests)
    }
    print ""

    # ---- Top IPs ----
    print "── Top " TOP_N " IPs by Requests ──────────"
    n = 0
    for (ip in ip_count) {
        sorted_keys[n] = ip
        sorted_vals[n] = ip_count[ip]
        n++
    }
    sort_desc(sorted_keys, sorted_vals, n)

    max_ip_count = (n > 0) ? sorted_vals[0] : 1
    limit = (n < TOP_N) ? n : TOP_N
    for (i = 0; i < limit; i++) {
        printf "  %-15s  %6d  %s  %s\n",
            sorted_keys[i], sorted_vals[i],
            bar_chart(sorted_vals[i], max_ip_count, 15),
            human_bytes(ip_bytes[sorted_keys[i]])
    }
    print ""

    # ---- Top Paths ----
    print "── Top " TOP_N " Paths ────────────────────"
    n = 0
    for (p in path_count) {
        sorted_keys[n] = p
        sorted_vals[n] = path_count[p]
        n++
    }
    sort_desc(sorted_keys, sorted_vals, n)

    limit = (n < TOP_N) ? n : TOP_N
    for (i = 0; i < limit; i++) {
        p = sorted_keys[i]
        printf "  %6d  %-40s  %s\n",
            sorted_vals[i],
            (length(p) > 40 ? substr(p, 1, 37) "..." : p),
            human_bytes(path_bytes[p])
    }
    print ""

    # ---- Hourly Distribution ----
    print "── Hourly Distribution ──────────────"
    max_hourly = 0
    for (h in hourly_count) {
        if (hourly_count[h] > max_hourly) max_hourly = hourly_count[h]
    }
    # Print sorted hours (00:00 to 23:00)
    for (h = 0; h < 24; h++) {
        hour_key = sprintf("%02d:00", h)
        count = (hour_key in hourly_count) ? hourly_count[hour_key] : 0
        printf "  %s  %s  %5d\n", hour_key, bar_chart(count, max_hourly, 30), count
    }
    print ""

    # ---- Bot Detection ----
    bot_total = 0
    for (ip in bot_hits) bot_total += bot_hits[ip]
    if (bot_total > 0) {
        print "── Bot Traffic ──────────────────────"
        printf "  Bot requests: %d (%s of total)\n",
            bot_total, percentage(bot_total, total_requests)
        for (ip in bot_hits) {
            if (bot_hits[ip] > 10) {
                printf "  %-15s  %d hits\n", ip, bot_hits[ip]
            }
        }
        print ""
    }

    # ---- Recent 5xx Errors ----
    if (status_5xx > 0) {
        print "── Recent Server Errors ─────────────"
        limit = (status_5xx < 10) ? status_5xx : 10
        for (i = error_count - limit + 1; i <= error_count; i++) {
            if (i in error_log) print "  " error_log[i]
        }
        print ""
    }

    print "======================================"
    printf "  Report generated: %s\n", strftime("%Y-%m-%d %H:%M:%S")
    print "======================================"
}
