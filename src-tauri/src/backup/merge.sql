CREATE TEMP TABLE category_mapping AS
SELECT source.id AS source, coalesce(by_id.id, by_name.id, source.id) AS target,
    by_id.id IS NULL AND by_name.id IS NULL AS is_new
FROM incoming.categories source
LEFT JOIN main.categories by_id ON by_id.id = source.id
LEFT JOIN main.categories by_name ON by_name.name = source.name COLLATE NOCASE;

INSERT INTO main.categories
SELECT source.* FROM incoming.categories source JOIN category_mapping mapping ON mapping.source = source.id WHERE mapping.is_new;

INSERT INTO main.recurrence_series (id,anchor_date,rule,title,category_id,priority,notes,due_offset,due_time,stop_on,stopped)
SELECT series.id,series.anchor_date,series.rule,series.title,mapping.target,series.priority,series.notes,series.due_offset,series.due_time,series.stop_on,series.stopped
FROM incoming.recurrence_series series LEFT JOIN category_mapping mapping ON mapping.source=series.category_id
WHERE NOT EXISTS (SELECT 1 FROM main.recurrence_series current WHERE current.id=series.id);

INSERT INTO main.tasks (id,title,created_at,category_id,priority,scheduled_date,due_date,due_time,notes,completed_at,trashed_at,rescheduled_on,manual_order,series_id,origin_series_id,occurrence_on,skipped_at)
SELECT task.id,task.title,task.created_at,mapping.target,task.priority,task.scheduled_date,task.due_date,task.due_time,task.notes,task.completed_at,task.trashed_at,task.rescheduled_on,task.manual_order,task.series_id,task.origin_series_id,task.occurrence_on,task.skipped_at
FROM incoming.tasks task LEFT JOIN category_mapping mapping ON mapping.source=task.category_id
WHERE task.id IN (SELECT id FROM added_tasks);

INSERT INTO main.subtasks
SELECT item.* FROM incoming.subtasks item WHERE item.task_id IN (SELECT id FROM added_tasks)
AND NOT EXISTS (SELECT 1 FROM main.subtasks current WHERE current.id=item.id);

INSERT INTO main.task_schedule_history (task_id,previous_date,next_date,changed_at,automatic)
SELECT task_id,previous_date,next_date,changed_at,automatic FROM incoming.task_schedule_history
WHERE task_id IN (SELECT id FROM added_tasks);

INSERT INTO main.task_completions (task_id,completed_at,planned_on,restored_at,task_key)
SELECT event.task_id,event.completed_at,event.planned_on,event.restored_at,event.task_key FROM incoming.task_completions event
WHERE event.task_id IN (SELECT id FROM added_tasks) OR (event.task_id IS NULL AND NOT EXISTS (
    SELECT 1 FROM main.task_completions current
    WHERE current.task_key IS event.task_key AND current.completed_at=event.completed_at
    AND current.planned_on IS event.planned_on AND current.restored_at IS event.restored_at
));

INSERT INTO main.task_alert_preferences
SELECT * FROM incoming.task_alert_preferences WHERE task_id IN (SELECT id FROM added_tasks);

INSERT INTO main.category_alert_preferences
SELECT source.* FROM incoming.category_alert_preferences source JOIN category_mapping mapping ON mapping.source=source.category_id WHERE mapping.is_new;

INSERT INTO main.statistics_plan_archive
SELECT source.* FROM incoming.statistics_plan_archive source
WHERE NOT EXISTS (SELECT 1 FROM main.statistics_plan_archive current WHERE current.task_key=source.task_key AND current.planned_on=source.planned_on);

CREATE TEMP TABLE batch_mapping(source INTEGER PRIMARY KEY, target INTEGER NOT NULL);

INSERT INTO main.app_settings(key,value)
SELECT key,value FROM incoming.app_settings WHERE key='statistics-started-at'
ON CONFLICT(key) DO UPDATE SET value=min(value,excluded.value);
