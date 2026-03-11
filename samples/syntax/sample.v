// V Language Syntax Highlighting Test
// A concurrent HTTP server with JSON handling and testing.

module main

import net.http
import json
import time
import os
import sync
import math
import arrays

// ============================================================
// Constants and types
// ============================================================

const (
	version         = '1.0.0'
	default_port    = 8080
	max_connections = 1000
	read_timeout    = 30 * time.second
	write_timeout   = 10 * time.second
)

enum Priority {
	low
	medium
	high
	critical
}

fn (p Priority) str() string {
	return match p {
		.low { 'low' }
		.medium { 'medium' }
		.high { 'high' }
		.critical { 'critical' }
	}
}

fn (p Priority) value() int {
	return match p {
		.low { 0 }
		.medium { 1 }
		.high { 2 }
		.critical { 3 }
	}
}

enum Status {
	open
	in_progress
	done
	cancelled
}

fn (s Status) icon() string {
	return match s {
		.open { '[ ]' }
		.in_progress { '[~]' }
		.done { '[x]' }
		.cancelled { '[-]' }
	}
}

struct Task {
	id          int
	title       string
	description string
	status      Status
	priority    Priority
	tags        []string
	created_at  time.Time
	updated_at  time.Time
mut:
	assignee string
}

fn (t Task) to_json() string {
	return json.encode(t)
}

fn task_from_json(data string) !Task {
	return json.decode(Task, data)!
}

// ============================================================
// Generic collections
// ============================================================

struct SortedList[T] {
mut:
	items  []T
	cmp_fn fn (T, T) int
}

fn new_sorted_list[T](cmp fn (T, T) int) SortedList[T] {
	return SortedList[T]{
		items:  []T{}
		cmp_fn: cmp
	}
}

fn (mut list SortedList[T]) insert(item T) {
	// Binary search for insertion point
	mut lo := 0
	mut hi := list.items.len
	for lo < hi {
		mid := lo + (hi - lo) / 2
		if list.cmp_fn(list.items[mid], item) < 0 {
			lo = mid + 1
		} else {
			hi = mid
		}
	}
	list.items.insert(lo, item)
}

fn (list SortedList[T]) find(predicate fn (T) bool) ?T {
	for item in list.items {
		if predicate(item) {
			return item
		}
	}
	return none
}

fn (list SortedList[T]) filter(predicate fn (T) bool) []T {
	mut result := []T{}
	for item in list.items {
		if predicate(item) {
			result << item
		}
	}
	return result
}

fn (list SortedList[T]) map_to[U](transform fn (T) U) []U {
	mut result := []U{cap: list.items.len}
	for item in list.items {
		result << transform(item)
	}
	return result
}

// ============================================================
// Task store with mutex
// ============================================================

struct TaskStore {
mut:
	tasks   map[int]Task
	next_id int = 1
	mu      sync.Mutex
}

fn new_task_store() &TaskStore {
	return &TaskStore{}
}

fn (mut store TaskStore) create(title string, priority Priority, tags []string) Task {
	store.mu.@lock()
	defer {
		store.mu.unlock()
	}

	now := time.now()
	task := Task{
		id:          store.next_id
		title:       title
		status:      .open
		priority:    priority
		tags:        tags
		created_at:  now
		updated_at:  now
	}
	store.tasks[store.next_id] = task
	store.next_id++
	return task
}

fn (mut store TaskStore) get(id int) !Task {
	store.mu.@lock()
	defer {
		store.mu.unlock()
	}

	return store.tasks[id] or { return error('task ${id} not found') }
}

fn (mut store TaskStore) update_status(id int, status Status) !Task {
	store.mu.@lock()
	defer {
		store.mu.unlock()
	}

	mut task := store.tasks[id] or { return error('task ${id} not found') }
	task = Task{
		...task
		status:     status
		updated_at: time.now()
	}
	store.tasks[id] = task
	return task
}

fn (mut store TaskStore) delete(id int) ! {
	store.mu.@lock()
	defer {
		store.mu.unlock()
	}

	if id !in store.tasks {
		return error('task ${id} not found')
	}
	store.tasks.delete(id)
}

fn (store TaskStore) list_all() []Task {
	mut tasks := []Task{cap: store.tasks.len}
	for _, task in store.tasks {
		tasks << task
	}
	tasks.sort_with_compare(fn (a &Task, b &Task) int {
		if a.priority.value() > b.priority.value() {
			return -1
		}
		if a.priority.value() < b.priority.value() {
			return 1
		}
		return 0
	})
	return tasks
}

fn (store TaskStore) filter_by_status(status Status) []Task {
	return store.list_all().filter(fn [status] (t Task) bool {
		return t.status == status
	})
}

fn (store TaskStore) filter_by_tag(tag string) []Task {
	return store.list_all().filter(fn [tag] (t Task) bool {
		return tag in t.tags
	})
}

// ============================================================
// Statistics
// ============================================================

struct Stats {
	total            int
	by_status        map[string]int
	by_priority      map[string]int
	completion_rate  f64
	avg_tags_per_task f64
}

fn compute_stats(tasks []Task) Stats {
	if tasks.len == 0 {
		return Stats{}
	}

	mut by_status := map[string]int{}
	mut by_priority := map[string]int{}
	mut total_tags := 0
	mut done_count := 0

	for task in tasks {
		by_status[task.status.icon()]++
		by_priority[task.priority.str()]++
		total_tags += task.tags.len
		if task.status == .done {
			done_count++
		}
	}

	return Stats{
		total:            tasks.len
		by_status:        by_status
		by_priority:      by_priority
		completion_rate:  f64(done_count) / f64(tasks.len) * 100.0
		avg_tags_per_task: f64(total_tags) / f64(tasks.len)
	}
}

// ============================================================
// String formatting helpers
// ============================================================

fn format_task(task Task) string {
	tags_str := if task.tags.len > 0 {
		' [${task.tags.join(", ")}]'
	} else {
		''
	}
	prio := match task.priority {
		.low { ' ' }
		.medium { '!' }
		.high { '!!' }
		.critical { '!!!' }
	}
	return '#${task.id} ${task.status.icon()} ${prio} ${task.title}${tags_str}'
}

fn format_report(stats Stats) string {
	mut lines := []string{}
	lines << '=== Task Report ==='
	lines << 'Total: ${stats.total}'
	lines << 'Completion: ${stats.completion_rate:.1f}%'
	lines << ''
	lines << 'By status:'
	for status, count in stats.by_status {
		bar := '#'.repeat(count)
		lines << '  ${status}: ${bar} (${count})'
	}
	lines << ''
	lines << 'By priority:'
	for priority, count in stats.by_priority {
		lines << '  ${priority}: ${count}'
	}
	return lines.join('\n')
}

// ============================================================
// Main
// ============================================================

fn main() {
	println('Task Manager v${version}')

	mut store := new_task_store()

	// Create sample tasks
	store.create('Implement syntax highlighting', .high, ['feature', 'syntax'])
	store.create('Fix cursor blinking', .low, ['bug'])
	store.create('Add split view', .medium, ['feature', 'ui'])
	store.create('Write documentation', .medium, ['docs'])
	store.create('Performance profiling', .high, ['perf'])

	// Complete one task
	store.update_status(2, .done) or { eprintln(err) }

	// Display all tasks
	tasks := store.list_all()
	for task in tasks {
		println('  ${format_task(task)}')
	}

	// Show stats
	stats := compute_stats(tasks)
	println('')
	println(format_report(stats))

	// Filter examples
	println('\nActive tasks:')
	for task in store.filter_by_status(.open) {
		println('  ${format_task(task)}')
	}

	println('\nFeature tasks:')
	for task in store.filter_by_tag('feature') {
		println('  ${format_task(task)}')
	}
}

// ============================================================
// Tests
// ============================================================

fn test_create_task() {
	mut store := new_task_store()
	task := store.create('Test task', .medium, ['test'])

	assert task.id == 1
	assert task.title == 'Test task'
	assert task.priority == .medium
	assert task.status == .open
	assert task.tags == ['test']
}

fn test_update_status() {
	mut store := new_task_store()
	store.create('Task 1', .low, [])

	updated := store.update_status(1, .done) or {
		assert false, 'should not fail'
		return
	}

	assert updated.status == .done
}

fn test_stats() {
	tasks := [
		Task{id: 1, title: 'A', status: .done, priority: .high, tags: ['a', 'b'], created_at: time.now(), updated_at: time.now()},
		Task{id: 2, title: 'B', status: .open, priority: .low, tags: ['a'], created_at: time.now(), updated_at: time.now()},
	]

	stats := compute_stats(tasks)
	assert stats.total == 2
	assert stats.completion_rate == 50.0
	assert stats.avg_tags_per_task == 1.5
}

fn test_sorted_list() {
	mut list := new_sorted_list[int](fn (a int, b int) int {
		return a - b
	})
	list.insert(3)
	list.insert(1)
	list.insert(2)

	assert list.items == [1, 2, 3]
}
