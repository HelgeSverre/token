//// Gleam Syntax Highlighting Test
//// A typed HTTP router with middleware, JSON handling, and pattern matching.

import gleam/bytes_builder
import gleam/dict.{type Dict}
import gleam/dynamic.{type Dynamic}
import gleam/float
import gleam/http.{type Method, Delete, Get, Patch, Post, Put}
import gleam/http/request.{type Request}
import gleam/http/response.{type Response}
import gleam/int
import gleam/io
import gleam/json.{type Json}
import gleam/list
import gleam/option.{type Option, None, Some}
import gleam/order
import gleam/result
import gleam/string

// ============================================================
// Types
// ============================================================

pub type AppError {
  NotFound(path: String)
  BadRequest(reason: String)
  Unauthorized
  InternalError(message: String)
}

pub type Context {
  Context(
    request_id: String,
    user: Option(User),
    start_time: Int,
  )
}

pub type User {
  User(id: Int, name: String, email: String, role: Role)
}

pub type Role {
  Admin
  Editor
  Viewer
}

pub type Task {
  Task(
    id: Int,
    title: String,
    description: Option(String),
    status: TaskStatus,
    priority: Priority,
    assignee_id: Option(Int),
    tags: List(String),
  )
}

pub type TaskStatus {
  Open
  InProgress
  Done
  Cancelled
}

pub type Priority {
  Low
  Medium
  High
  Critical
}

pub type Route {
  Route(
    method: Method,
    path: List(String),
    handler: fn(Request(String), Context) -> Result(Response(String), AppError),
  )
}

pub type Middleware =
  fn(
    fn(Request(String), Context) -> Result(Response(String), AppError),
  ) -> fn(Request(String), Context) -> Result(Response(String), AppError)

// ============================================================
// Task operations (pure)
// ============================================================

pub fn create_task(
  tasks: List(Task),
  title: String,
  priority: Priority,
  tags: List(String),
) -> #(List(Task), Task) {
  let id = case list.last(tasks) {
    Ok(last) -> last.id + 1
    Error(_) -> 1
  }

  let task = Task(
    id: id,
    title: title,
    description: None,
    status: Open,
    priority: priority,
    assignee_id: None,
    tags: tags,
  )

  #(list.append(tasks, task), task)
}

pub fn find_task(tasks: List(Task), id: Int) -> Result(Task, AppError) {
  tasks
  |> list.find(fn(t) { t.id == id })
  |> result.replace_error(NotFound("Task #" <> int.to_string(id)))
}

pub fn update_task_status(
  tasks: List(Task),
  id: Int,
  status: TaskStatus,
) -> Result(List(Task), AppError) {
  let updated = list.map(tasks, fn(task) {
    case task.id == id {
      True -> Task(..task, status: status)
      False -> task
    }
  })

  // Verify the task existed
  case list.any(tasks, fn(t) { t.id == id }) {
    True -> Ok(updated)
    False -> Error(NotFound("Task #" <> int.to_string(id)))
  }
}

pub fn filter_tasks(
  tasks: List(Task),
  status: Option(TaskStatus),
  tag: Option(String),
) -> List(Task) {
  tasks
  |> list.filter(fn(task) {
    let status_match = case status {
      Some(s) -> task.status == s
      None -> True
    }

    let tag_match = case tag {
      Some(t) -> list.contains(task.tags, t)
      None -> True
    }

    status_match && tag_match
  })
  |> list.sort(fn(a, b) { compare_priority(b.priority, a.priority) })
}

fn compare_priority(a: Priority, b: Priority) -> order.Order {
  let to_int = fn(p) {
    case p {
      Low -> 0
      Medium -> 1
      High -> 2
      Critical -> 3
    }
  }
  int.compare(to_int(a), to_int(b))
}

// ============================================================
// JSON serialization
// ============================================================

pub fn task_to_json(task: Task) -> Json {
  json.object([
    #("id", json.int(task.id)),
    #("title", json.string(task.title)),
    #("description", case task.description {
      Some(d) -> json.string(d)
      None -> json.null()
    }),
    #("status", json.string(status_to_string(task.status))),
    #("priority", json.string(priority_to_string(task.priority))),
    #("assignee_id", case task.assignee_id {
      Some(id) -> json.int(id)
      None -> json.null()
    }),
    #("tags", json.array(task.tags, json.string)),
  ])
}

pub fn tasks_to_json(tasks: List(Task)) -> String {
  json.array(tasks, task_to_json)
  |> json.to_string
}

fn status_to_string(status: TaskStatus) -> String {
  case status {
    Open -> "open"
    InProgress -> "in_progress"
    Done -> "done"
    Cancelled -> "cancelled"
  }
}

fn priority_to_string(priority: Priority) -> String {
  case priority {
    Low -> "low"
    Medium -> "medium"
    High -> "high"
    Critical -> "critical"
  }
}

// ============================================================
// Middleware
// ============================================================

pub fn logging_middleware() -> Middleware {
  fn(handler) {
    fn(req: Request(String), ctx: Context) {
      io.println(
        "[" <> ctx.request_id <> "] "
        <> string.uppercase(http.method_to_string(req.method))
        <> " " <> req.path,
      )

      let result = handler(req, ctx)

      case result {
        Ok(resp) ->
          io.println(
            "[" <> ctx.request_id <> "] -> " <> int.to_string(resp.status),
          )
        Error(err) ->
          io.println(
            "[" <> ctx.request_id <> "] -> ERROR: " <> error_to_string(err),
          )
      }

      result
    }
  }
}

pub fn auth_middleware(require_role: Role) -> Middleware {
  fn(handler) {
    fn(req: Request(String), ctx: Context) {
      case ctx.user {
        None -> Error(Unauthorized)
        Some(user) -> {
          case has_permission(user.role, require_role) {
            True -> handler(req, ctx)
            False -> Error(Unauthorized)
          }
        }
      }
    }
  }
}

fn has_permission(user_role: Role, required: Role) -> Bool {
  case user_role, required {
    Admin, _ -> True
    Editor, Editor | Editor, Viewer -> True
    Viewer, Viewer -> True
    _, _ -> False
  }
}

// ============================================================
// Router
// ============================================================

pub fn route(
  req: Request(String),
  ctx: Context,
  routes: List(Route),
) -> Response(String) {
  let path_segments =
    req.path
    |> string.split("/")
    |> list.filter(fn(s) { s != "" })

  let result =
    routes
    |> list.find_map(fn(r) {
      case r.method == req.method && match_path(r.path, path_segments) {
        True -> Ok(r.handler(req, ctx))
        False -> Error(Nil)
      }
    })
    |> result.flatten

  case result {
    Ok(resp) -> resp
    Error(NotFound(path)) -> error_response(404, "Not found: " <> path)
    Error(BadRequest(reason)) -> error_response(400, reason)
    Error(Unauthorized) -> error_response(401, "Unauthorized")
    Error(InternalError(msg)) -> error_response(500, msg)
  }
}

fn match_path(pattern: List(String), actual: List(String)) -> Bool {
  case pattern, actual {
    [], [] -> True
    [":" <> _, ..rest_pattern], [_, ..rest_actual] ->
      match_path(rest_pattern, rest_actual)
    [p, ..rest_pattern], [a, ..rest_actual] if p == a ->
      match_path(rest_pattern, rest_actual)
    _, _ -> False
  }
}

fn error_response(status: Int, message: String) -> Response(String) {
  let body =
    json.object([
      #("error", json.string(message)),
      #("status", json.int(status)),
    ])
    |> json.to_string

  response.new(status)
  |> response.set_body(body)
  |> response.set_header("content-type", "application/json")
}

fn error_to_string(error: AppError) -> String {
  case error {
    NotFound(p) -> "NotFound: " <> p
    BadRequest(r) -> "BadRequest: " <> r
    Unauthorized -> "Unauthorized"
    InternalError(m) -> "InternalError: " <> m
  }
}

// ============================================================
// Stats
// ============================================================

pub type Stats {
  Stats(
    total: Int,
    by_status: Dict(String, Int),
    by_priority: Dict(String, Int),
    completion_rate: Float,
  )
}

pub fn compute_stats(tasks: List(Task)) -> Stats {
  let total = list.length(tasks)
  let done = list.count(tasks, fn(t) { t.status == Done })

  let by_status =
    tasks
    |> list.group(fn(t) { status_to_string(t.status) })
    |> dict.map_values(fn(_, v) { list.length(v) })

  let by_priority =
    tasks
    |> list.group(fn(t) { priority_to_string(t.priority) })
    |> dict.map_values(fn(_, v) { list.length(v) })

  let completion_rate = case total {
    0 -> 0.0
    _ -> int.to_float(done) /. int.to_float(total) *. 100.0
  }

  Stats(
    total: total,
    by_status: by_status,
    by_priority: by_priority,
    completion_rate: completion_rate,
  )
}

pub fn stats_to_json(stats: Stats) -> String {
  json.object([
    #("total", json.int(stats.total)),
    #("completion_rate", json.float(stats.completion_rate)),
    #("by_status", dict_to_json(stats.by_status)),
    #("by_priority", dict_to_json(stats.by_priority)),
  ])
  |> json.to_string
}

fn dict_to_json(d: Dict(String, Int)) -> Json {
  d
  |> dict.to_list
  |> list.map(fn(pair) { #(pair.0, json.int(pair.1)) })
  |> json.object
}
