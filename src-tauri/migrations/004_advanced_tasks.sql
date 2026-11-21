CREATE TABLE recurrence_series (
    id TEXT NOT NULL PRIMARY KEY,
    anchor_date TEXT NOT NULL,
    rule TEXT NOT NULL,
    title TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    priority TEXT NOT NULL CHECK (priority IN ('high', 'medium', 'low')),
    notes TEXT NOT NULL,
    due_offset INTEGER,
    due_time TEXT,
    stop_on TEXT,
    stopped INTEGER NOT NULL DEFAULT 0 CHECK (stopped IN (0, 1))
) STRICT;

ALTER TABLE tasks ADD COLUMN series_id TEXT REFERENCES recurrence_series(id);
ALTER TABLE tasks ADD COLUMN origin_series_id TEXT REFERENCES recurrence_series(id);
ALTER TABLE tasks ADD COLUMN occurrence_on TEXT;
ALTER TABLE tasks ADD COLUMN skipped_at TEXT;

CREATE UNIQUE INDEX one_active_occurrence ON tasks(series_id)
    WHERE series_id IS NOT NULL AND completed_at IS NULL AND trashed_at IS NULL AND skipped_at IS NULL;
CREATE INDEX task_series_history ON tasks(series_id, occurrence_on);

CREATE TABLE subtasks (
    id TEXT NOT NULL PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    position INTEGER NOT NULL,
    completed_at TEXT
) STRICT;
CREATE INDEX subtasks_order ON subtasks(task_id, position, id);

CREATE TABLE task_completions (
    id INTEGER PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    completed_at TEXT NOT NULL,
    planned_on TEXT,
    restored_at TEXT
) STRICT;
CREATE INDEX task_completion_dates ON task_completions(completed_at);
CREATE INDEX task_completion_task ON task_completions(task_id);
INSERT INTO task_completions(task_id, completed_at, planned_on)
SELECT id, completed_at, scheduled_date FROM tasks WHERE completed_at IS NOT NULL;
