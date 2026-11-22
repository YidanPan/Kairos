ALTER TABLE task_completions ADD COLUMN task_key TEXT;
UPDATE task_completions SET task_key = coalesce(task_id, 'completion:' || id);

INSERT INTO app_settings(key, value)
SELECT 'statistics-started-at', min(stamp) FROM (
    SELECT min(created_at) AS stamp FROM tasks
    UNION ALL SELECT min(completed_at) FROM task_completions
)
HAVING min(stamp) IS NOT NULL;

CREATE TRIGGER record_statistics_start_on_task_insert
AFTER INSERT ON tasks
BEGIN
    INSERT INTO app_settings(key, value) VALUES ('statistics-started-at', NEW.created_at)
    ON CONFLICT(key) DO UPDATE SET value = min(value, excluded.value);
END;

CREATE TABLE statistics_plan_archive (
    task_key TEXT NOT NULL,
    planned_on TEXT NOT NULL,
    skipped INTEGER NOT NULL CHECK (skipped IN (0, 1)),
    PRIMARY KEY (task_key, planned_on)
) STRICT;

CREATE TRIGGER archive_statistics_before_task_delete
BEFORE DELETE ON tasks
BEGIN
    INSERT OR IGNORE INTO statistics_plan_archive (task_key, planned_on, skipped)
    SELECT OLD.id, OLD.scheduled_date, OLD.skipped_at IS NOT NULL
    WHERE OLD.scheduled_date IS NOT NULL AND (
        OLD.completed_at IS NOT NULL OR OLD.skipped_at IS NOT NULL
        OR OLD.scheduled_date < coalesce(date(OLD.trashed_at, 'localtime'), date('now', 'localtime'))
    );

    INSERT OR IGNORE INTO statistics_plan_archive (task_key, planned_on, skipped)
    SELECT OLD.id, previous_date, 0 FROM task_schedule_history
    WHERE task_id = OLD.id AND automatic = 1;
END;
