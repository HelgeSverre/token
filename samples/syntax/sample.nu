# Nushell Syntax Highlighting Test
# A system monitoring toolkit with structured data pipelines.

# ============================================================
# Configuration and constants
# ============================================================

const VERSION = "1.0.0"
const MAX_LOG_LINES = 1000
const ALERT_CPU_THRESHOLD = 80.0
const ALERT_MEM_THRESHOLD = 90.0

# Color theme for output
let colors = {
    ok: "green"
    warn: "yellow"
    error: "red"
    info: "cyan"
    muted: "dark_gray"
}

# ============================================================
# Custom commands
# ============================================================

# Get system overview as a structured table
def system-overview [] {
    let cpu = (sys cpu | get cpu_usage | math avg)
    let mem = (sys mem)
    let disk = (sys disks | where mount != "/System/Volumes/Data")

    let mem_pct = ($mem.used / $mem.total * 100 | math round --precision 1)
    let disk_info = ($disk | each {|d|
        let pct = ($d.used / $d.total * 100 | math round --precision 1)
        {
            mount: $d.mount
            total: ($d.total | into filesize)
            used: ($d.used | into filesize)
            free: ($d.free | into filesize)
            pct: $pct
        }
    })

    {
        hostname: (hostname)
        uptime: (sys host | get uptime)
        cpu_usage: ($cpu | math round --precision 1)
        memory: {
            total: ($mem.total | into filesize)
            used: ($mem.used | into filesize)
            free: ($mem.free | into filesize)
            percent: $mem_pct
        }
        disks: $disk_info
        load_avg: (sys cpu | get cpu_usage | first 3)
    }
}

# Process monitor - find top processes by resource usage
def "proc top" [
    --count (-n): int = 10     # Number of processes to show
    --sort-by (-s): string = "cpu"  # Sort by: cpu, mem, or name
] {
    let procs = (ps | where cpu > 0.0)

    let sorted = match $sort_by {
        "cpu" => { $procs | sort-by cpu --reverse }
        "mem" => { $procs | sort-by mem --reverse }
        "name" => { $procs | sort-by name }
        _ => {
            print $"(ansi $colors.error)Unknown sort field: ($sort_by)(ansi reset)"
            return
        }
    }

    $sorted
    | first $count
    | select pid name cpu mem
    | each {|row|
        let cpu_color = if $row.cpu > $ALERT_CPU_THRESHOLD {
            $colors.error
        } else if $row.cpu > 50.0 {
            $colors.warn
        } else {
            $colors.ok
        }

        $row | update cpu {|r|
            $"(ansi $cpu_color)($r.cpu | math round --precision 1)%(ansi reset)"
        }
    }
}

# Port scanner - check which ports are listening
def "port scan" [
    host: string = "localhost"  # Host to scan
    --range (-r): range = 1..1024  # Port range
    --timeout (-t): duration = 1sec  # Connection timeout
] {
    $range | each {|port|
        let result = (do { ^nc -z -w 1 $host ($port | into string) } | complete)
        if $result.exit_code == 0 {
            { port: $port, status: "open", host: $host }
        }
    } | compact | sort-by port
}

# ============================================================
# Log analysis
# ============================================================

# Parse and analyze log files
def "log analyze" [
    path: path                      # Path to log file
    --format (-f): string = "auto"  # Log format: auto, json, nginx, syslog
    --since (-s): datetime = (date now | $in - 1hr)  # Only logs after this time
    --level (-l): string = "all"    # Filter by level: all, error, warn, info, debug
] {
    let content = (open $path | lines | last $MAX_LOG_LINES)

    let parsed = match $format {
        "json" => {
            $content | each {|line|
                try { $line | from json } catch { null }
            } | compact
        }
        "nginx" => {
            $content | parse '{ip} - {user} [{timestamp}] "{method} {path} {proto}" {status} {bytes}'
            | each {|row|
                $row | update status { into int } | update bytes { into int }
            }
        }
        _ => {
            # Auto-detect: try JSON first, then plain text
            $content | each {|line|
                try {
                    $line | from json
                } catch {
                    { raw: $line, level: "unknown" }
                }
            }
        }
    }

    # Filter by level
    let filtered = if $level == "all" {
        $parsed
    } else {
        $parsed | where level == $level
    }

    # Generate summary
    let summary = {
        total_lines: ($filtered | length)
        by_level: ($filtered | group-by level | transpose key value | each {|g|
            { level: $g.key, count: ($g.value | length) }
        })
        error_rate: (
            let errors = ($filtered | where level in ["error", "fatal"] | length)
            let total = ($filtered | length)
            if $total > 0 { ($errors / $total * 100 | math round --precision 2) } else { 0 }
        )
    }

    print $"(ansi $colors.info)Log Analysis: ($path)(ansi reset)"
    print $"  Lines analyzed: ($summary.total_lines)"
    print $"  Error rate: ($summary.error_rate)%"
    print ""
    print "By level:"
    $summary.by_level | table

    $filtered
}

# ============================================================
# Docker helpers
# ============================================================

# List running containers with resource usage
def "dk ps" [] {
    ^docker ps --format '{{json .}}' | lines | each { from json } | select Names Image Status Ports
}

# Container resource stats as structured data
def "dk stats" [] {
    ^docker stats --no-stream --format '{{json .}}'
    | lines
    | each { from json }
    | select Name CPUPerc MemUsage MemPerc NetIO BlockIO
    | each {|row|
        $row
        | update CPUPerc { str replace "%" "" | into float }
        | update MemPerc { str replace "%" "" | into float }
    }
    | sort-by CPUPerc --reverse
}

# Quick container logs with optional follow
def "dk logs" [
    container: string   # Container name or ID
    --tail (-n): int = 100
    --follow (-f)       # Follow log output
] {
    if $follow {
        ^docker logs --follow --tail $tail $container
    } else {
        ^docker logs --tail $tail $container | lines
    }
}

# ============================================================
# Git helpers
# ============================================================

# Enhanced git log with structured output
def "git-log" [
    --count (-n): int = 20
    --author (-a): string = ""
] {
    let format = '{"hash":"%h","author":"%an","date":"%ai","subject":"%s"}'
    mut cmd = [log $"--format=($format)" $"-($count)" --no-merges]

    if $author != "" {
        $cmd = ($cmd | append $"--author=($author)")
    }

    ^git ...$cmd | lines | each { from json } | each {|row|
        $row | update date { into datetime }
    }
}

# Show commit frequency by day of week
def "git activity" [] {
    git-log --count 500
    | each {|c| $c.date | format date "%A" }
    | uniq --count
    | sort-by count --reverse
    | rename day commits
}

# ============================================================
# File system helpers
# ============================================================

# Find large files in a directory
def "find-large" [
    path: path = "."           # Directory to search
    --min-size (-s): filesize = 10MB  # Minimum file size
    --count (-n): int = 20     # Max results
] {
    glob $"($path)/**/*"
    | each {|f|
        let info = ($f | path expand | ls $in | first)
        if $info.size >= $min_size {
            {
                path: ($f | path relative-to $path)
                size: $info.size
                modified: $info.modified
            }
        }
    }
    | compact
    | sort-by size --reverse
    | first $count
}

# Directory size breakdown
def "du-pretty" [path: path = "."] {
    ls $path
    | each {|entry|
        let size = if ($entry.type == "dir") {
            glob $"($entry.name)/**/*" | each {|f|
                try { ls $f | first | get size } catch { 0 }
            } | math sum
        } else {
            $entry.size
        }
        {
            name: ($entry.name | path basename)
            type: $entry.type
            size: ($size | into filesize)
        }
    }
    | sort-by size --reverse
}

# ============================================================
# Report generation
# ============================================================

def "report generate" [
    --format (-f): string = "table"  # Output: table, json, csv, markdown
] {
    let overview = (system-overview)
    let top_procs = (proc top --count 5)

    match $format {
        "json" => { $overview | to json }
        "csv" => { $top_procs | to csv }
        "markdown" => {
            $"# System Report\n\n"
            + $"**Host:** ($overview.hostname)\n"
            + $"**CPU:** ($overview.cpu_usage)%\n"
            + $"**Memory:** ($overview.memory.percent)%\n\n"
            + $"## Top Processes\n\n"
            + ($top_procs | to md)
        }
        _ => {
            print $"(ansi $colors.info)System Report - ($overview.hostname)(ansi reset)"
            print $"  CPU:    ($overview.cpu_usage)%"
            print $"  Memory: ($overview.memory.percent)%"
            print ""
            print "Top processes:"
            $top_procs | table
        }
    }
}

# ============================================================
# Main entry point
# ============================================================

def main [] {
    print $"(ansi $colors.info)System Monitor v($VERSION)(ansi reset)"
    print ""
    report generate
}
