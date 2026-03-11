-- SQL Syntax Highlighting Test
-- A comprehensive schema and query collection for a project management system.

-- Schema definition with constraints
CREATE TABLE IF NOT EXISTS users (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username        VARCHAR(50) NOT NULL UNIQUE,
    email           VARCHAR(255) NOT NULL UNIQUE,
    password_hash   CHAR(60) NOT NULL,
    display_name    VARCHAR(100),
    avatar_url      TEXT,
    role            VARCHAR(20) NOT NULL DEFAULT 'member'
                    CHECK (role IN ('admin', 'manager', 'member', 'viewer')),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at   TIMESTAMPTZ
);

CREATE TABLE projects (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name            VARCHAR(200) NOT NULL,
    slug            VARCHAR(200) NOT NULL UNIQUE,
    description     TEXT,
    owner_id        BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    visibility      VARCHAR(10) NOT NULL DEFAULT 'private'
                    CHECK (visibility IN ('public', 'private', 'internal')),
    archived_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tasks (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    project_id      BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id       BIGINT REFERENCES tasks(id) ON DELETE SET NULL,
    title           VARCHAR(500) NOT NULL,
    description     TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'open'
                    CHECK (status IN ('open', 'in_progress', 'review', 'done', 'cancelled')),
    priority        SMALLINT NOT NULL DEFAULT 0
                    CHECK (priority BETWEEN 0 AND 4),
    assignee_id     BIGINT REFERENCES users(id) ON DELETE SET NULL,
    due_date        DATE,
    estimated_hours NUMERIC(6, 2),
    actual_hours    NUMERIC(6, 2),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

CREATE TABLE task_tags (
    task_id         BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tag             VARCHAR(50) NOT NULL,
    PRIMARY KEY (task_id, tag)
);

CREATE TABLE comments (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    task_id         BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    author_id       BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body            TEXT NOT NULL,
    edited          BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_tasks_project_status ON tasks(project_id, status);
CREATE INDEX idx_tasks_assignee ON tasks(assignee_id) WHERE assignee_id IS NOT NULL;
CREATE INDEX idx_tasks_due_date ON tasks(due_date) WHERE due_date IS NOT NULL AND status != 'done';
CREATE INDEX idx_comments_task ON comments(task_id, created_at DESC);

-- Trigger function for updated_at
CREATE OR REPLACE FUNCTION update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_users_updated
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

CREATE TRIGGER trg_tasks_updated
    BEFORE UPDATE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_timestamp();

-- View: active project summary
CREATE OR REPLACE VIEW project_summary AS
SELECT
    p.id,
    p.name,
    p.slug,
    u.display_name AS owner_name,
    COUNT(DISTINCT t.id) AS total_tasks,
    COUNT(DISTINCT t.id) FILTER (WHERE t.status = 'done') AS completed_tasks,
    COUNT(DISTINCT t.id) FILTER (WHERE t.status = 'in_progress') AS active_tasks,
    COUNT(DISTINCT t.assignee_id) AS team_size,
    ROUND(
        100.0 * COUNT(t.id) FILTER (WHERE t.status = 'done') /
        NULLIF(COUNT(t.id), 0),
        1
    ) AS completion_pct,
    MIN(t.due_date) FILTER (WHERE t.status != 'done') AS next_deadline
FROM projects p
JOIN users u ON u.id = p.owner_id
LEFT JOIN tasks t ON t.project_id = p.id
WHERE p.archived_at IS NULL
GROUP BY p.id, p.name, p.slug, u.display_name;

-- CTE: task hierarchy (recursive)
WITH RECURSIVE task_tree AS (
    -- Base case: root tasks
    SELECT
        t.id,
        t.title,
        t.parent_id,
        t.status,
        t.priority,
        0 AS depth,
        ARRAY[t.id] AS path,
        t.title::TEXT AS full_path
    FROM tasks t
    WHERE t.parent_id IS NULL
      AND t.project_id = 1

    UNION ALL

    -- Recursive case: child tasks
    SELECT
        t.id,
        t.title,
        t.parent_id,
        t.status,
        t.priority,
        tt.depth + 1,
        tt.path || t.id,
        tt.full_path || ' > ' || t.title
    FROM tasks t
    JOIN task_tree tt ON tt.id = t.parent_id
    WHERE tt.depth < 10  -- prevent infinite recursion
)
SELECT
    REPEAT('  ', depth) || title AS indented_title,
    status,
    priority,
    depth,
    full_path
FROM task_tree
ORDER BY path;

-- Window functions: team workload analysis
SELECT
    u.display_name,
    COUNT(t.id) AS assigned_tasks,
    SUM(COALESCE(t.estimated_hours, 0)) AS total_estimated,
    SUM(COALESCE(t.actual_hours, 0)) AS total_actual,
    RANK() OVER (ORDER BY COUNT(t.id) DESC) AS workload_rank,
    ROUND(
        AVG(t.estimated_hours) OVER (PARTITION BY u.id),
        1
    ) AS avg_estimate,
    ARRAY_AGG(DISTINCT t.status ORDER BY t.status) AS status_breakdown,
    COUNT(t.id) FILTER (WHERE t.due_date < CURRENT_DATE AND t.status != 'done') AS overdue
FROM users u
LEFT JOIN tasks t ON t.assignee_id = u.id
WHERE u.is_active = TRUE
GROUP BY u.id, u.display_name
HAVING COUNT(t.id) > 0
ORDER BY workload_rank;

-- Lateral join: latest comment per task
SELECT
    t.id AS task_id,
    t.title,
    lc.body AS latest_comment,
    lc.author_name,
    lc.commented_at
FROM tasks t
CROSS JOIN LATERAL (
    SELECT
        c.body,
        u.display_name AS author_name,
        c.created_at AS commented_at
    FROM comments c
    JOIN users u ON u.id = c.author_id
    WHERE c.task_id = t.id
    ORDER BY c.created_at DESC
    LIMIT 1
) lc
WHERE t.project_id = 1
ORDER BY lc.commented_at DESC;

-- Insert with conflict handling
INSERT INTO users (username, email, password_hash, display_name, role)
VALUES
    ('alice', 'alice@example.com', '$2b$12$hash1', 'Alice Smith', 'admin'),
    ('bob', 'bob@example.com', '$2b$12$hash2', 'Bob Jones', 'member'),
    ('carol', 'carol@example.com', '$2b$12$hash3', 'Carol White', 'manager')
ON CONFLICT (email)
DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at = NOW()
RETURNING id, username, email;

-- Complex update with subquery
UPDATE tasks
SET status = 'cancelled',
    updated_at = NOW()
WHERE project_id IN (
    SELECT id FROM projects
    WHERE archived_at IS NOT NULL
      AND archived_at < NOW() - INTERVAL '90 days'
)
AND status NOT IN ('done', 'cancelled');

-- Stored procedure
CREATE OR REPLACE FUNCTION assign_task(
    p_task_id BIGINT,
    p_assignee_id BIGINT,
    p_comment TEXT DEFAULT NULL
)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
DECLARE
    v_current_assignee BIGINT;
    v_task_exists BOOLEAN;
BEGIN
    SELECT assignee_id, TRUE INTO v_current_assignee, v_task_exists
    FROM tasks
    WHERE id = p_task_id
    FOR UPDATE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Task % not found', p_task_id;
    END IF;

    UPDATE tasks
    SET assignee_id = p_assignee_id,
        status = CASE
            WHEN status = 'open' THEN 'in_progress'
            ELSE status
        END
    WHERE id = p_task_id;

    IF p_comment IS NOT NULL THEN
        INSERT INTO comments (task_id, author_id, body)
        VALUES (p_task_id, p_assignee_id, p_comment);
    END IF;

    RETURN TRUE;
END;
$$;
