#!/usr/bin/env tclsh

# Tcl Syntax Highlighting Test
# A configuration management system with namespaces, OO, and event handling.

package require Tcl 8.6
package require TclOO

# ============================================================
# Constants and configuration
# ============================================================

namespace eval ::config {
    variable version "1.0.0"
    variable debug 0
    variable defaults [dict create \
        host     "localhost" \
        port     8080 \
        timeout  30000 \
        retries  3 \
        log_level "info" \
        data_dir  [file join [file dirname [info script]] data] \
    ]

    # Log levels
    variable log_levels [dict create \
        debug 0 \
        info  1 \
        warn  2 \
        error 3 \
        fatal 4 \
    ]
}

# ============================================================
# Logging
# ============================================================

namespace eval ::log {
    variable channel stderr

    proc timestamp {} {
        clock format [clock seconds] -format "%Y-%m-%d %H:%M:%S"
    }

    proc log {level message args} {
        variable channel
        set min_level [dict get $::config::log_levels $::config::defaults]
        set msg_level [dict get $::config::log_levels $level]

        if {$msg_level >= $min_level} {
            set prefix "[timestamp] \[$level\]"
            if {[llength $args] > 0} {
                set message [format $message {*}$args]
            }
            puts $channel "$prefix $message"
        }
    }

    proc debug {message args} { log debug $message {*}$args }
    proc info  {message args} { log info  $message {*}$args }
    proc warn  {message args} { log warn  $message {*}$args }
    proc error {message args} { log error $message {*}$args }

    namespace export debug info warn error
}

# ============================================================
# Utility procedures
# ============================================================

proc assert {condition {message ""}} {
    if {![uplevel 1 [list expr $condition]]} {
        if {$message eq ""} {
            set message "Assertion failed: $condition"
        }
        error $message
    }
}

proc try_with_retry {max_retries delay body} {
    set last_error ""
    for {set attempt 1} {$attempt <= $max_retries} {incr attempt} {
        try {
            set result [uplevel 1 $body]
            return $result
        } on error {msg opts} {
            set last_error $msg
            ::log::warn "Attempt %d/%d failed: %s" $attempt $max_retries $msg
            if {$attempt < $max_retries} {
                after $delay
            }
        }
    }
    error "All $max_retries attempts failed. Last error: $last_error"
}

proc dict_get_or {d key default} {
    if {[dict exists $d $key]} {
        return [dict get $d $key]
    }
    return $default
}

# Deep merge two dicts
proc dict_merge {base overlay} {
    set result $base
    dict for {key value} $overlay {
        if {[dict exists $base $key] && [string is list $value] && [llength $value] % 2 == 0} {
            set base_val [dict get $base $key]
            if {[string is list $base_val] && [llength $base_val] % 2 == 0} {
                dict set result $key [dict_merge $base_val $value]
                continue
            }
        }
        dict set result $key $value
    }
    return $result
}

# ============================================================
# TclOO Classes
# ============================================================

# Base event emitter
oo::class create EventEmitter {
    variable listeners

    constructor {} {
        set listeners [dict create]
    }

    method on {event callback} {
        if {![dict exists $listeners $event]} {
            dict set listeners $event [list]
        }
        dict lappend listeners $event $callback
        return [self]
    }

    method off {event {callback ""}} {
        if {$callback eq ""} {
            dict unset listeners $event
        } elseif {[dict exists $listeners $event]} {
            set cbs [dict get $listeners $event]
            set idx [lsearch -exact $cbs $callback]
            if {$idx >= 0} {
                dict set listeners $event [lreplace $cbs $idx $idx]
            }
        }
    }

    method emit {event args} {
        if {[dict exists $listeners $event]} {
            foreach callback [dict get $listeners $event] {
                try {
                    uplevel #0 $callback $args
                } on error {msg} {
                    ::log::error "Event handler error for '%s': %s" $event $msg
                }
            }
        }
    }
}

# Task with validation and state machine
oo::class create Task {
    superclass EventEmitter

    variable id title description status priority tags created_at

    constructor {task_id task_title args} {
        next ;# Call EventEmitter constructor

        set id $task_id
        set title $task_title
        set description [dict_get_or $args description ""]
        set status "open"
        set priority [dict_get_or $args priority "medium"]
        set tags [dict_get_or $args tags {}]
        set created_at [clock seconds]

        # Validate priority
        if {$priority ni {low medium high critical}} {
            error "Invalid priority: $priority"
        }
    }

    method id {} { return $id }
    method title {} { return $title }
    method status {} { return $status }
    method priority {} { return $priority }
    method tags {} { return $tags }

    method transition {new_status} {
        # State machine: validate transitions
        set valid_transitions [dict create \
            open        {in_progress cancelled} \
            in_progress {open done cancelled} \
            done        {open} \
            cancelled   {open} \
        ]

        set allowed [dict get $valid_transitions $status]
        if {$new_status ni $allowed} {
            error "Cannot transition from '$status' to '$new_status'"
        }

        set old_status $status
        set status $new_status

        my emit status_changed $id $old_status $new_status
        ::log::info "Task %s: %s -> %s" $id $old_status $new_status

        return [self]
    }

    method add_tag {tag} {
        if {$tag ni $tags} {
            lappend tags $tag
            my emit tag_added $id $tag
        }
        return [self]
    }

    method remove_tag {tag} {
        set idx [lsearch -exact $tags $tag]
        if {$idx >= 0} {
            set tags [lreplace $tags $idx $idx]
            my emit tag_removed $id $tag
        }
        return [self]
    }

    method to_dict {} {
        return [dict create \
            id $id \
            title $title \
            description $description \
            status $status \
            priority $priority \
            tags $tags \
            created_at $created_at \
        ]
    }

    method format {} {
        set icon [dict get {
            open "[ ]" in_progress "[~]" done "[x]" cancelled "[-]"
        } $status]

        set prio_mark [dict get {
            low " " medium "!" high "!!" critical "!!!"
        } $priority]

        set tag_str ""
        if {[llength $tags] > 0} {
            set tag_str " \[[join $tags ", "]\]"
        }

        return "#$id $icon $prio_mark $title$tag_str"
    }
}

# Task store
oo::class create TaskStore {
    superclass EventEmitter

    variable tasks next_id

    constructor {} {
        next
        set tasks [dict create]
        set next_id 1
    }

    method create {title args} {
        set task [Task new $next_id $title {*}$args]
        dict set tasks $next_id $task
        incr next_id

        # Forward events
        $task on status_changed [list [self] _on_task_changed]
        my emit task_created [$task id]

        return $task
    }

    method get {id} {
        if {![dict exists $tasks $id]} {
            error "Task $id not found"
        }
        return [dict get $tasks $id]
    }

    method delete {id} {
        if {![dict exists $tasks $id]} {
            error "Task $id not found"
        }
        set task [dict get $tasks $id]
        $task destroy
        dict unset tasks $id
        my emit task_deleted $id
    }

    method list {{filter_proc ""}} {
        set result [list]
        dict for {id task} $tasks {
            if {$filter_proc eq "" || [apply $filter_proc $task]} {
                lappend result $task
            }
        }
        return $result
    }

    method count {} {
        return [dict size $tasks]
    }

    method stats {} {
        set by_status [dict create open 0 in_progress 0 done 0 cancelled 0]
        set by_priority [dict create low 0 medium 0 high 0 critical 0]

        dict for {id task} $tasks {
            dict incr by_status [$task status]
            dict incr by_priority [$task priority]
        }

        set total [dict size $tasks]
        set done [dict get $by_status done]
        set rate [expr {$total > 0 ? double($done) / $total * 100 : 0.0}]

        return [dict create \
            total $total \
            by_status $by_status \
            by_priority $by_priority \
            completion_rate [format "%.1f" $rate] \
        ]
    }

    method _on_task_changed {args} {
        my emit task_changed {*}$args
    }
}

# ============================================================
# String processing with regexp
# ============================================================

proc parse_duration {str} {
    set total_ms 0
    set patterns {
        {(\d+)h}  3600000
        {(\d+)m}  60000
        {(\d+)s}  1000
        {(\d+)ms} 1
    }

    foreach {pattern multiplier} $patterns {
        if {[regexp $pattern $str -> value]} {
            incr total_ms [expr {$value * $multiplier}]
        }
    }

    return $total_ms
}

proc format_duration {ms} {
    set parts [list]
    foreach {unit divisor} {h 3600000 m 60000 s 1000} {
        set value [expr {$ms / $divisor}]
        if {$value > 0} {
            lappend parts "${value}${unit}"
            set ms [expr {$ms % $divisor}]
        }
    }
    if {[llength $parts] == 0} {
        return "0s"
    }
    return [join $parts ""]
}

# ============================================================
# Main
# ============================================================

proc main {} {
    ::log::info "Task Manager v%s" $::config::version

    # Create store
    set store [TaskStore new]

    # Listen for events
    $store on task_created {apply {{id} {
        ::log::info "Created task #%s" $id
    }}}

    $store on task_changed {apply {{id old new} {
        ::log::info "Task #%s changed: %s -> %s" $id $old $new
    }}}

    # Create tasks
    set t1 [$store create "Implement syntax highlighting" \
        priority high tags {feature syntax}]
    set t2 [$store create "Fix cursor blinking" \
        priority low tags {bug}]
    set t3 [$store create "Add split view" \
        priority medium tags {feature ui}]

    # Transition states
    $t1 transition in_progress
    $t2 transition in_progress
    $t2 transition done

    # Display
    puts "\nAll tasks:"
    foreach task [$store list] {
        puts "  [$task format]"
    }

    # Stats
    puts "\nStatistics:"
    set stats [$store stats]
    dict for {key value} $stats {
        puts "  $key: $value"
    }

    # Cleanup
    $store destroy
}

# Run if executed directly
if {[::info script] eq $::argv0} {
    main
}
