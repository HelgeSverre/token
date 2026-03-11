"""
Pony Syntax Highlighting Test
A concurrent task scheduler with reference capabilities and actors.
"""

use "collections"
use "time"
use "promises"
use "format"

// ============================================================
// Primitives (singleton types)
// ============================================================

primitive Low    fun string(): String => "low"
primitive Medium fun string(): String => "medium"
primitive High   fun string(): String => "high"
primitive Critical fun string(): String => "critical"

type Priority is (Low | Medium | High | Critical)

primitive Open        fun string(): String => "open"
primitive InProgress  fun string(): String => "in_progress"
primitive Done        fun string(): String => "done"
primitive Cancelled   fun string(): String => "cancelled"

type Status is (Open | InProgress | Done | Cancelled)

// ============================================================
// Value types (classes with reference capabilities)
// ============================================================

class val Task
  """A task with value semantics (deeply immutable once created)."""
  let id: USize
  let title: String
  let description: String
  let status: Status
  let priority: Priority
  let tags: Array[String] val
  let created_at: U64

  new val create(
    id': USize,
    title': String,
    priority': Priority = Medium,
    tags': Array[String] val = recover val Array[String] end,
    description': String = "",
    status': Status = Open,
    created_at': U64 = 0)
  =>
    id = id'
    title = title'
    description = description'
    status = status'
    priority = priority'
    tags = tags'
    created_at = created_at'

  fun with_status(new_status: Status): Task =>
    """Return a new task with updated status."""
    Task.create(id, title, priority, tags, description, new_status, created_at)

  fun priority_value(): USize =>
    match priority
    | Low => 0
    | Medium => 1
    | High => 2
    | Critical => 3
    end

  fun status_icon(): String =>
    match status
    | Open => "[ ]"
    | InProgress => "[~]"
    | Done => "[x]"
    | Cancelled => "[-]"
    end

  fun priority_icon(): String =>
    match priority
    | Low => " "
    | Medium => "!"
    | High => "!!"
    | Critical => "!!!"
    end

  fun format(): String =>
    let tag_str = if tags.size() > 0 then
      " [" + ", ".join(tags.values()) + "]"
    else
      ""
    end
    "#" + id.string() + " " + status_icon() + " " + priority_icon()
      + " " + title + tag_str

// ============================================================
// Stats (immutable value)
// ============================================================

class val TaskStats
  let total: USize
  let by_status: Map[String, USize] val
  let by_priority: Map[String, USize] val
  let completion_rate: F64

  new val create(
    total': USize,
    by_status': Map[String, USize] val,
    by_priority': Map[String, USize] val,
    completion_rate': F64)
  =>
    total = total'
    by_status = by_status'
    by_priority = by_priority'
    completion_rate = completion_rate'

  fun format(): String =>
    var out = "=== Statistics ===\n"
    out = out + "Total: " + total.string() + "\n"
    out = out + "Completion: " + Format.float[F64](
      where x = completion_rate, prec = 1) + "%\n"
    out

// ============================================================
// Actor: TaskStore (concurrent, isolated state)
// ============================================================

actor TaskStore
  """
  Thread-safe task store using actor model.
  All mutations go through message passing - no locks needed.
  """
  var _tasks: Map[USize, Task]
  var _next_id: USize
  let _env: Env

  new create(env: Env) =>
    _tasks = Map[USize, Task]
    _next_id = 1
    _env = env

  be add_task(title: String, priority: Priority,
              tags: Array[String] val, callback: {(Task)} val) =>
    """Create a task and notify via callback."""
    let task = Task.create(_next_id, title, priority, tags)
    _tasks(_next_id) = task
    _next_id = _next_id + 1
    callback(task)

  be update_status(id: USize, new_status: Status,
                   callback: {(Task | None)} val) =>
    """Update task status with state machine validation."""
    try
      let task = _tasks(id)?
      if _valid_transition(task.status, new_status) then
        let updated = task.with_status(new_status)
        _tasks(id) = updated
        callback(updated)
      else
        _env.err.print("Invalid transition from " + task.status.string()
          + " to " + new_status.string())
        callback(None)
      end
    else
      _env.err.print("Task " + id.string() + " not found")
      callback(None)
    end

  be remove_task(id: USize, callback: {(Bool)} val) =>
    try
      _tasks.remove(id)?
      callback(true)
    else
      callback(false)
    end

  be get_all(callback: {(Array[Task] val)} val) =>
    """Return all tasks sorted by priority."""
    let tasks = recover val
      let arr = Array[Task]
      for task in _tasks.values() do
        arr.push(task)
      end
      // Sort by priority descending
      Sort[Array[Task], Task](arr)
      arr
    end
    callback(tasks)

  be get_stats(callback: {(TaskStats)} val) =>
    var total: USize = 0
    let by_status = recover trn Map[String, USize] end
    let by_priority = recover trn Map[String, USize] end
    var done_count: USize = 0

    for task in _tasks.values() do
      total = total + 1
      let s = task.status.string()
      let p = task.priority.string()
      by_status(s) = by_status.get_or_else(s, 0) + 1
      by_priority(p) = by_priority.get_or_else(p, 0) + 1
      match task.status
      | Done => done_count = done_count + 1
      end
    end

    let rate: F64 = if total > 0 then
      (done_count.f64() / total.f64()) * 100.0
    else
      0.0
    end

    let stats = TaskStats.create(
      total,
      consume by_status,
      consume by_priority,
      rate)
    callback(stats)

  fun _valid_transition(from: Status, to: Status): Bool =>
    match (from, to)
    | (Open, InProgress) => true
    | (Open, Cancelled) => true
    | (InProgress, Open) => true
    | (InProgress, Done) => true
    | (InProgress, Cancelled) => true
    | (Done, Open) => true
    | (Cancelled, Open) => true
    else
      false
    end

// ============================================================
// Actor: Worker (processes tasks concurrently)
// ============================================================

actor Worker
  let _id: USize
  let _env: Env
  var _processed: USize

  new create(id: USize, env: Env) =>
    _id = id
    _env = env
    _processed = 0

  be process(task: Task, callback: {(USize, Task)} val) =>
    """Simulate processing a task."""
    _env.out.print("  Worker " + _id.string() + " processing: " + task.title)
    _processed = _processed + 1
    callback(_id, task)

  be get_processed(callback: {(USize, USize)} val) =>
    callback(_id, _processed)

// ============================================================
// Sort interface for Tasks
// ============================================================

primitive _TaskCompare is Compare[Task]
  fun apply(a: Task, b: Task): (Less | Equal | Greater) =>
    if a.priority_value() > b.priority_value() then Less
    elseif a.priority_value() < b.priority_value() then Greater
    else Equal
    end

// ============================================================
// Main actor
// ============================================================

actor Main
  let _env: Env
  let _store: TaskStore

  new create(env: Env) =>
    _env = env
    _store = TaskStore(env)

    env.out.print("Task Scheduler v1.0\n")

    // Create tasks
    let tags1 = recover val ["feature"; "syntax"] end
    let tags2 = recover val ["bug"] end
    let tags3 = recover val ["feature"; "ui"] end
    let tags4 = recover val ["docs"] end
    let tags5 = recover val ["perf"] end

    _store.add_task("Implement syntax highlighting", High, tags1,
      {(task: Task)(env) => env.out.print("  Created: " + task.format())})
    _store.add_task("Fix cursor blinking", Low, tags2,
      {(task: Task)(env) => env.out.print("  Created: " + task.format())})
    _store.add_task("Add split view", Medium, tags3,
      {(task: Task)(env) => env.out.print("  Created: " + task.format())})
    _store.add_task("Write documentation", Medium, tags4,
      {(task: Task)(env) => env.out.print("  Created: " + task.format())})
    _store.add_task("Performance profiling", High, tags5,
      {(task: Task)(env) => env.out.print("  Created: " + task.format())})

    // Update some statuses
    _store.update_status(1, InProgress,
      {(result: (Task | None))(env) =>
        match result
        | let t: Task => env.out.print("  Updated: " + t.format())
        | None => env.out.print("  Update failed")
        end
      })

    _store.update_status(2, Done,
      {(result: (Task | None))(env) =>
        match result
        | let t: Task => env.out.print("  Updated: " + t.format())
        | None => env.out.print("  Update failed")
        end
      })

    // Print stats
    _store.get_stats(
      {(stats: TaskStats)(env) =>
        env.out.print("\n" + stats.format())
      })
