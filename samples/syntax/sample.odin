// Odin Syntax Highlighting Test
// A data-oriented task manager with SOA layout and custom allocators.

package main

import "core:fmt"
import "core:mem"
import "core:strings"
import "core:slice"
import "core:time"
import "core:math"
import "core:os"
import "core:unicode/utf8"

// ============================================================
// Enums and constants
// ============================================================

MAX_TASKS :: 1024
MAX_TAGS_PER_TASK :: 8
MAX_TAG_LENGTH :: 32

Priority :: enum u8 {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

Status :: enum u8 {
    Open = 0,
    In_Progress = 1,
    Done = 2,
    Cancelled = 3,
}

priority_string :: proc(p: Priority) -> string {
    switch p {
    case .Low:      return "low"
    case .Medium:   return "medium"
    case .High:     return "high"
    case .Critical: return "critical"
    }
    return "unknown"
}

status_icon :: proc(s: Status) -> string {
    switch s {
    case .Open:        return "[ ]"
    case .In_Progress: return "[~]"
    case .Done:        return "[x]"
    case .Cancelled:   return "[-]"
    }
    return "[?]"
}

priority_icon :: proc(p: Priority) -> string {
    switch p {
    case .Low:      return " "
    case .Medium:   return "!"
    case .High:     return "!!"
    case .Critical: return "!!!"
    }
    return "?"
}

// ============================================================
// SOA (Structure of Arrays) task storage
// ============================================================

Tag :: [MAX_TAG_LENGTH]u8

Task_Store :: struct {
    // SOA layout for cache-friendly iteration
    ids:          [MAX_TASKS]u32,
    titles:       [MAX_TASKS][256]u8,
    title_lens:   [MAX_TASKS]u8,
    statuses:     [MAX_TASKS]Status,
    priorities:   [MAX_TASKS]Priority,
    tags:         [MAX_TASKS][MAX_TAGS_PER_TASK]Tag,
    tag_counts:   [MAX_TASKS]u8,
    created_at:   [MAX_TASKS]time.Time,

    count:   u32,
    next_id: u32,
}

init_store :: proc() -> Task_Store {
    store: Task_Store
    store.next_id = 1
    return store
}

// ============================================================
// Task operations
// ============================================================

create_task :: proc(
    store: ^Task_Store,
    title: string,
    priority: Priority = .Medium,
    tags: []string = {},
) -> (u32, bool) {
    if store.count >= MAX_TASKS {
        fmt.eprintln("Error: task store full")
        return 0, false
    }

    idx := store.count
    id := store.next_id

    store.ids[idx] = id
    store.statuses[idx] = .Open
    store.priorities[idx] = priority
    store.created_at[idx] = time.now()
    store.tag_counts[idx] = 0

    // Copy title
    title_bytes := transmute([]u8)title
    title_len := min(len(title_bytes), 255)
    copy(store.titles[idx][:title_len], title_bytes[:title_len])
    store.title_lens[idx] = u8(title_len)

    // Copy tags
    for tag, i in tags {
        if i >= MAX_TAGS_PER_TASK do break
        tag_bytes := transmute([]u8)tag
        tag_len := min(len(tag_bytes), MAX_TAG_LENGTH - 1)
        copy(store.tags[idx][i][:tag_len], tag_bytes[:tag_len])
        store.tag_counts[idx] += 1
    }

    store.count += 1
    store.next_id += 1

    return id, true
}

find_task_index :: proc(store: ^Task_Store, id: u32) -> (u32, bool) {
    for i in 0..<store.count {
        if store.ids[i] == id {
            return i, true
        }
    }
    return 0, false
}

update_status :: proc(store: ^Task_Store, id: u32, new_status: Status) -> bool {
    idx, found := find_task_index(store, id)
    if !found {
        fmt.eprintf("Error: task %d not found\n", id)
        return false
    }

    current := store.statuses[idx]

    // Validate transition
    valid := false
    switch current {
    case .Open:
        valid = new_status == .In_Progress || new_status == .Cancelled
    case .In_Progress:
        valid = new_status == .Open || new_status == .Done || new_status == .Cancelled
    case .Done:
        valid = new_status == .Open
    case .Cancelled:
        valid = new_status == .Open
    }

    if !valid {
        fmt.eprintf("Error: invalid transition %v -> %v\n", current, new_status)
        return false
    }

    store.statuses[idx] = new_status
    return true
}

delete_task :: proc(store: ^Task_Store, id: u32) -> bool {
    idx, found := find_task_index(store, id)
    if !found do return false

    // Swap with last element (O(1) removal)
    last := store.count - 1
    if idx != last {
        store.ids[idx] = store.ids[last]
        store.titles[idx] = store.titles[last]
        store.title_lens[idx] = store.title_lens[last]
        store.statuses[idx] = store.statuses[last]
        store.priorities[idx] = store.priorities[last]
        store.tags[idx] = store.tags[last]
        store.tag_counts[idx] = store.tag_counts[last]
        store.created_at[idx] = store.created_at[last]
    }

    store.count -= 1
    return true
}

// ============================================================
// Query and filter
// ============================================================

get_title :: proc(store: ^Task_Store, idx: u32) -> string {
    return string(store.titles[idx][:store.title_lens[idx]])
}

get_tag :: proc(store: ^Task_Store, task_idx: u32, tag_idx: u8) -> string {
    tag := store.tags[task_idx][tag_idx]
    // Find null terminator
    len := 0
    for i in 0..<MAX_TAG_LENGTH {
        if tag[i] == 0 do break
        len += 1
    }
    return string(tag[:len])
}

filter_by_status :: proc(store: ^Task_Store, status: Status, allocator := context.allocator) -> [dynamic]u32 {
    result := make([dynamic]u32, allocator)
    for i in 0..<store.count {
        if store.statuses[i] == status {
            append(&result, store.ids[i])
        }
    }
    return result
}

filter_by_tag :: proc(store: ^Task_Store, tag: string, allocator := context.allocator) -> [dynamic]u32 {
    result := make([dynamic]u32, allocator)
    tag_bytes := transmute([]u8)tag

    for i in 0..<store.count {
        for j in 0..<store.tag_counts[i] {
            task_tag := get_tag(store, i, j)
            if task_tag == tag {
                append(&result, store.ids[i])
                break
            }
        }
    }
    return result
}

// ============================================================
// Statistics
// ============================================================

Stats :: struct {
    total:           u32,
    by_status:       [len(Status)]u32,
    by_priority:     [len(Priority)]u32,
    completion_rate: f64,
}

compute_stats :: proc(store: ^Task_Store) -> Stats {
    stats: Stats
    stats.total = store.count

    for i in 0..<store.count {
        stats.by_status[store.statuses[i]] += 1
        stats.by_priority[store.priorities[i]] += 1
    }

    done := stats.by_status[Status.Done]
    if store.count > 0 {
        stats.completion_rate = f64(done) / f64(store.count) * 100.0
    }

    return stats
}

print_stats :: proc(stats: Stats) {
    fmt.println("\n=== Statistics ===")
    fmt.printf("Total: %d\n", stats.total)
    fmt.printf("Completion: %.1f%%\n", stats.completion_rate)
    fmt.println("\nBy status:")
    for s in Status {
        fmt.printf("  %v: %d\n", s, stats.by_status[s])
    }
    fmt.println("\nBy priority:")
    for p in Priority {
        fmt.printf("  %v: %d\n", p, stats.by_priority[p])
    }
}

// ============================================================
// Display
// ============================================================

print_task :: proc(store: ^Task_Store, idx: u32) {
    id := store.ids[idx]
    title := get_title(store, idx)
    status := status_icon(store.statuses[idx])
    prio := priority_icon(store.priorities[idx])

    fmt.printf("  #%d %s %s %s", id, status, prio, title)

    if store.tag_counts[idx] > 0 {
        fmt.print(" [")
        for j in 0..<store.tag_counts[idx] {
            if j > 0 do fmt.print(", ")
            fmt.print(get_tag(store, idx, j))
        }
        fmt.print("]")
    }
    fmt.println()
}

print_all_tasks :: proc(store: ^Task_Store) {
    fmt.println("All tasks:")

    // Sort indices by priority (descending)
    indices := make([dynamic]u32, context.temp_allocator)
    for i in 0..<store.count {
        append(&indices, i)
    }

    slice.sort_by(indices[:], proc(a, b: u32) -> bool {
        // Access through closure... Odin doesn't have closures,
        // so we'd normally use a different pattern
        return true
    })

    for idx in indices {
        print_task(store, idx)
    }
}

// ============================================================
// Main
// ============================================================

main :: proc() {
    // Use tracking allocator in debug mode
    when ODIN_DEBUG {
        track: mem.Tracking_Allocator
        mem.tracking_allocator_init(&track, context.allocator)
        context.allocator = mem.tracking_allocator(&track)

        defer {
            if len(track.allocation_map) > 0 {
                fmt.eprintf("=== %v allocations not freed ===\n", len(track.allocation_map))
                for _, entry in track.allocation_map {
                    fmt.eprintf("  %v bytes at %v\n", entry.size, entry.location)
                }
            }
        }
    }

    fmt.println("Task Manager v1.0\n")

    store := init_store()

    // Create tasks
    create_task(&store, "Implement syntax highlighting", .High, {"feature", "syntax"})
    create_task(&store, "Fix cursor blinking", .Low, {"bug"})
    create_task(&store, "Add split view", .Medium, {"feature", "ui"})
    create_task(&store, "Write documentation", .Medium, {"docs"})
    create_task(&store, "Performance profiling", .High, {"perf"})

    // Update statuses
    update_status(&store, 1, .In_Progress)
    update_status(&store, 2, .Done)

    // Display
    print_all_tasks(&store)

    // Stats
    stats := compute_stats(&store)
    print_stats(stats)

    // Filter
    fmt.println("\nFeature tasks:")
    feature_ids := filter_by_tag(&store, "feature", context.temp_allocator)
    for id in feature_ids {
        idx, _ := find_task_index(&store, id)
        print_task(&store, idx)
    }
}
