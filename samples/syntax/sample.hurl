# Hurl Syntax Highlighting Test
# HTTP request testing for a task management API.

# ============================================================
# Health check
# ============================================================

GET http://localhost:8080/health
HTTP 200
[Asserts]
header "Content-Type" == "application/json"
jsonpath "$.status" == "ok"
jsonpath "$.version" matches /^\d+\.\d+\.\d+$/
jsonpath "$.uptime" > 0
duration < 500

# ============================================================
# Authentication
# ============================================================

# Register a new user
POST http://localhost:8080/api/auth/register
Content-Type: application/json
{
    "username": "testuser",
    "email": "test@example.com",
    "password": "SecureP@ss123",
    "display_name": "Test User"
}
HTTP 201
[Captures]
user_id: jsonpath "$.id"
[Asserts]
jsonpath "$.username" == "testuser"
jsonpath "$.email" == "test@example.com"
jsonpath "$.role" == "member"
jsonpath "$.id" isString

# Login and capture token
POST http://localhost:8080/api/auth/login
Content-Type: application/json
{
    "email": "test@example.com",
    "password": "SecureP@ss123"
}
HTTP 200
[Captures]
auth_token: jsonpath "$.token"
refresh_token: jsonpath "$.refresh_token"
[Asserts]
jsonpath "$.token" isString
jsonpath "$.expires_in" == 3600
cookie "session" exists

# Verify token works
GET http://localhost:8080/api/auth/me
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.id" == {{user_id}}
jsonpath "$.username" == "testuser"

# ============================================================
# Project CRUD
# ============================================================

# Create a project
POST http://localhost:8080/api/projects
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "name": "Token Editor",
    "description": "A minimal text editor in Rust",
    "visibility": "private"
}
HTTP 201
[Captures]
project_id: jsonpath "$.id"
project_slug: jsonpath "$.slug"
[Asserts]
jsonpath "$.name" == "Token Editor"
jsonpath "$.slug" == "token-editor"
jsonpath "$.visibility" == "private"
jsonpath "$.owner.id" == {{user_id}}

# Get project by slug
GET http://localhost:8080/api/projects/{{project_slug}}
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.id" == {{project_id}}
jsonpath "$.task_count" == 0
jsonpath "$.completion_percentage" == 0.0

# ============================================================
# Task management
# ============================================================

# Create tasks with different priorities
POST http://localhost:8080/api/projects/{{project_id}}/tasks
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "title": "Implement syntax highlighting",
    "description": "Add tree-sitter based syntax highlighting for 20+ languages",
    "priority": "high",
    "tags": ["feature", "syntax"],
    "due_date": "2024-12-31T00:00:00Z"
}
HTTP 201
[Captures]
task1_id: jsonpath "$.id"
[Asserts]
jsonpath "$.status" == "open"
jsonpath "$.priority" == "high"
jsonpath "$.tags" count == 2
jsonpath "$.tags[0]" == "feature"

POST http://localhost:8080/api/projects/{{project_id}}/tasks
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "title": "Fix cursor blinking",
    "priority": "low",
    "tags": ["bug"]
}
HTTP 201
[Captures]
task2_id: jsonpath "$.id"

POST http://localhost:8080/api/projects/{{project_id}}/tasks
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "title": "Add split view support",
    "priority": "medium",
    "tags": ["feature", "ui"],
    "parent_id": "{{task1_id}}"
}
HTTP 201
[Captures]
task3_id: jsonpath "$.id"

# List tasks with filtering
GET http://localhost:8080/api/projects/{{project_id}}/tasks?status=open&priority=high&sort=priority&direction=desc
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.tasks" count >= 1
jsonpath "$.tasks[0].priority" == "high"
jsonpath "$.total_count" >= 1

# List tasks with tag filter
GET http://localhost:8080/api/projects/{{project_id}}/tasks?tag=bug
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.tasks" count == 1
jsonpath "$.tasks[0].title" == "Fix cursor blinking"

# Update task status
PATCH http://localhost:8080/api/projects/{{project_id}}/tasks/{{task1_id}}
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "status": "in_progress"
}
HTTP 200
[Asserts]
jsonpath "$.status" == "in_progress"

# Complete a task
PATCH http://localhost:8080/api/projects/{{project_id}}/tasks/{{task2_id}}
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "status": "done"
}
HTTP 200
[Asserts]
jsonpath "$.status" == "done"
jsonpath "$.completed_at" isString

# ============================================================
# Comments
# ============================================================

# Add a comment
POST http://localhost:8080/api/tasks/{{task1_id}}/comments
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "body": "Started working on this. The tree-sitter integration is looking good!"
}
HTTP 201
[Captures]
comment_id: jsonpath "$.id"
[Asserts]
jsonpath "$.author.id" == {{user_id}}
jsonpath "$.body" contains "tree-sitter"

# List comments
GET http://localhost:8080/api/tasks/{{task1_id}}/comments
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.comments" count >= 1

# ============================================================
# Statistics
# ============================================================

# Get project stats
GET http://localhost:8080/api/projects/{{project_id}}/stats
Authorization: Bearer {{auth_token}}
HTTP 200
[Asserts]
jsonpath "$.total_tasks" == 3
jsonpath "$.status_counts.open" >= 1
jsonpath "$.status_counts.done" == 1
jsonpath "$.completion_percentage" > 0

# ============================================================
# Error cases
# ============================================================

# Unauthorized access
GET http://localhost:8080/api/projects/{{project_id}}/tasks
HTTP 401
[Asserts]
jsonpath "$.error" exists

# Not found
GET http://localhost:8080/api/projects/nonexistent
Authorization: Bearer {{auth_token}}
HTTP 404

# Bad request - missing required field
POST http://localhost:8080/api/projects/{{project_id}}/tasks
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "priority": "high"
}
HTTP 400
[Asserts]
jsonpath "$.error" contains "title"

# Invalid priority value
POST http://localhost:8080/api/projects/{{project_id}}/tasks
Authorization: Bearer {{auth_token}}
Content-Type: application/json
{
    "title": "Test task",
    "priority": "super-urgent"
}
HTTP 400

# ============================================================
# Cleanup
# ============================================================

# Delete task
DELETE http://localhost:8080/api/projects/{{project_id}}/tasks/{{task3_id}}
Authorization: Bearer {{auth_token}}
HTTP 204

# Verify deletion
GET http://localhost:8080/api/projects/{{project_id}}/tasks/{{task3_id}}
Authorization: Bearer {{auth_token}}
HTTP 404
