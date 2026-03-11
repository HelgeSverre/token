#!/usr/bin/sed -f

# sed Syntax Highlighting Test
# A log file colorizer and transformer.
# Transforms nginx access logs into a formatted, readable report.
#
# Usage: cat access.log | sed -f sample.sed
#
# Input format:
#   192.168.1.1 - user [10/Oct/2024:13:55:36 -0700] "GET /api HTTP/1.1" 200 2326

# ============================================================
# Header: Insert report title at the beginning
# ============================================================

1{
    i\
====================================\
  Access Log Report\
====================================\

}

# ============================================================
# Remove entries we don't care about
# ============================================================

# Delete empty lines
/^[[:space:]]*$/d

# Delete comment lines
/^#/d

# Delete health check noise
/\/health\b/d
/\/ping\b/d
/\/favicon\.ico/d

# Delete static asset requests (optional - comment out to keep)
# /\.(css|js|png|jpg|gif|svg|woff|woff2)\b/d

# ============================================================
# Normalize and clean up
# ============================================================

# Remove HTTP version from request
s/"(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS) ([^ ]*) HTTP\/[0-9.]+"/"\1 \2"/

# Trim trailing whitespace
s/[[:space:]]*$//

# ============================================================
# Status code classification
# ============================================================

# Mark 2xx responses as SUCCESS
/\" 2[0-9][0-9] /{
    s/\" 2\([0-9][0-9]\) /\" [OK:\2] /
}

# Mark 3xx responses as REDIRECT
/\" 3[0-9][0-9] /{
    s/\" 3\([0-9][0-9]\) /\" [REDIR:\3] /
}

# Mark 4xx responses as CLIENT ERROR
/\" 4[0-9][0-9] /{
    s/\" 4\([0-9][0-9]\) /\" [ERR:\4] /
}

# Mark 404 specifically
/\" 404 /{
    s/\" 404 /\" [NOT_FOUND] /
}

# Mark 5xx responses as SERVER ERROR (highlight these)
/\" 5[0-9][0-9] /{
    s/\" 5\([0-9][0-9]\) /\" [FATAL:5\1] /
    # Prefix with warning marker
    s/^/>>> /
}

# ============================================================
# Extract and reformat key fields
# ============================================================

# Reformat: IP [timestamp] METHOD PATH STATUS BYTES
# From: IP - user [timestamp] "METHOD PATH" STATUS BYTES ...
s/^\([0-9.]*\) - [^ ]* \[\([^]]*\)\] "\([A-Z]*\) \([^ ]*\)" \([^ ]*\) \([0-9]*\).*/\1  \2  \3 \4  \5  \6 bytes/

# ============================================================
# Human-readable byte sizes
# ============================================================

# Convert large byte counts to KB (rough approximation)
# Match 4+ digit byte counts and add KB marker
/[0-9][0-9][0-9][0-9][0-9]* bytes$/{
    # This is a simplification - sed can't do arithmetic easily
    s/\([0-9]*\) bytes$/\1 bytes (~KB)/
}

# ============================================================
# Timestamp reformatting
# ============================================================

# Shorten month names
s/January/Jan/g
s/February/Feb/g
s/March/Mar/g
s/April/Apr/g
s/May/May/g
s/June/Jun/g
s/July/Jul/g
s/August/Aug/g
s/September/Sep/g
s/October/Oct/g
s/November/Nov/g
s/December/Dec/g

# Remove timezone offset from timestamps
s/\([0-9][0-9]:[0-9][0-9]:[0-9][0-9]\) [-+][0-9]*/\1/

# ============================================================
# Path categorization
# ============================================================

# Mark API endpoints
s| /api/\([^ ]*\)| /api/\1 [API]|

# Mark authentication endpoints
s| /auth/\([^ ]*\)| /auth/\1 [AUTH]|
s| /login| /login [AUTH]|
s| /logout| /logout [AUTH]|

# Mark admin endpoints
s| /admin/\([^ ]*\)| /admin/\1 [ADMIN]|

# Mark websocket connections
s| /ws\b| /ws [WS]|

# ============================================================
# IP address classification
# ============================================================

# Mark private/internal IPs
/^10\.\|^172\.1[6-9]\.\|^172\.2[0-9]\.\|^172\.3[01]\.\|^192\.168\./{
    s/^/[INT] /
}

# Mark localhost
/^127\.0\.0\.1/{
    s/^/[LOCAL] /
}

# ============================================================
# Separator between entries for readability
# ============================================================

# Add a thin separator every 10 lines
0~10{
    a\
---
}

# ============================================================
# Footer: Append summary note at the end
# ============================================================

${
    a\
\
====================================\
  End of Report\
  [INT] = Internal IP\
  [API] = API endpoint\
  [AUTH] = Authentication\
  >>> = Server Error (5xx)\
====================================
}
