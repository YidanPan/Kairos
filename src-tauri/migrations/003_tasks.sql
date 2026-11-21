CREATE TABLE categories (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) > 0),
    color TEXT NOT NULL
) STRICT;

ALTER TABLE tasks ADD COLUMN category_id TEXT REFERENCES categories(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN priority TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('high', 'medium', 'low'));
ALTER TABLE tasks ADD COLUMN scheduled_date TEXT;
ALTER TABLE tasks ADD COLUMN due_date TEXT;
ALTER TABLE tasks ADD COLUMN due_time TEXT;
ALTER TABLE tasks ADD COLUMN notes TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN completed_at TEXT;
ALTER TABLE tasks ADD COLUMN trashed_at TEXT;
ALTER TABLE tasks ADD COLUMN rescheduled_on TEXT;
ALTER TABLE tasks ADD COLUMN manual_order INTEGER NOT NULL DEFAULT 0;

CREATE TABLE task_schedule_history (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    previous_date TEXT NOT NULL,
    next_date TEXT,
    changed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    automatic INTEGER NOT NULL CHECK (automatic IN (0, 1))
) STRICT;

CREATE INDEX task_schedule_history_task ON task_schedule_history(task_id, id);
CREATE INDEX tasks_category ON tasks(category_id);
