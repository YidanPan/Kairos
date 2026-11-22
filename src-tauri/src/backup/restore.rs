use rusqlite::{Connection, Transaction};

use super::{error, Result, DIRECTORY_KEY};

const NEW_TASKS: &str = "SELECT b.id FROM incoming.tasks b WHERE NOT EXISTS (
    SELECT 1 FROM main.tasks current WHERE current.id = b.id OR (
        b.series_id IS NOT NULL AND current.series_id = b.series_id AND (
            (b.occurrence_on IS NOT NULL AND current.occurrence_on = b.occurrence_on)
            OR (b.completed_at IS NULL AND b.trashed_at IS NULL AND b.skipped_at IS NULL
                AND current.completed_at IS NULL AND current.trashed_at IS NULL AND current.skipped_at IS NULL)
        )
    )
)";

pub(super) fn count_new_tasks(current: &Connection) -> Result<u64> {
    current
        .query_row(&format!("SELECT count(*) FROM ({NEW_TASKS})"), [], |row| {
            row.get(0)
        })
        .map_err(error)
}

pub(super) fn replace(database: &Transaction<'_>, directory: Option<&str>) -> Result<()> {
    // Parents are inserted before children. Trigger output left by old task deletion is cleared.
    const TABLES: &[&str] = &[
        "categories",
        "recurrence_series",
        "tasks",
        "subtasks",
        "task_schedule_history",
        "task_completions",
        "task_alert_preferences",
        "category_alert_preferences",
        "reminder_batches",
        "reminder_receipts",
        "statistics_plan_archive",
        "app_assets",
        "app_settings",
    ];
    for table in TABLES.iter().rev() {
        database
            .execute(&format!("DELETE FROM main.{table}"), [])
            .map_err(error)?;
    }
    database
        .execute("DELETE FROM main.statistics_plan_archive", [])
        .map_err(error)?;
    for table in TABLES {
        if *table == "app_settings" {
            database
                .execute("DELETE FROM main.app_settings", [])
                .map_err(error)?;
        }
        database
            .execute(
                &format!("INSERT INTO main.{table} SELECT * FROM incoming.{table}"),
                [],
            )
            .map_err(error)?;
    }
    database
        .execute(
            "DELETE FROM main.app_settings WHERE key=?1",
            [DIRECTORY_KEY],
        )
        .map_err(error)?;
    if let Some(directory) = directory {
        database
            .execute(
                "INSERT INTO main.app_settings(key,value) VALUES (?1,?2)",
                [DIRECTORY_KEY, directory],
            )
            .map_err(error)?;
    }
    Ok(())
}

pub(super) fn merge(database: &Transaction<'_>) -> Result<()> {
    database
        .execute_batch(&format!("CREATE TEMP TABLE added_tasks AS {NEW_TASKS};"))
        .map_err(error)?;
    database
        .execute_batch(include_str!("merge.sql"))
        .map_err(error)?;
    let mut source_batches = database.prepare("SELECT id FROM incoming.reminder_batches WHERE task_id IN (SELECT id FROM added_tasks) ORDER BY id").map_err(error)?;
    let ids = source_batches
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(error)?;
    for id in ids {
        let id = id.map_err(error)?;
        database.execute("INSERT INTO main.reminder_batches(task_id,created_at,first_due_at,count,silent,native_status,dismissed,snoozed_for)
            SELECT task_id,created_at,first_due_at,count,silent,native_status,dismissed,snoozed_for FROM incoming.reminder_batches WHERE id=?1", [id]).map_err(error)?;
        database
            .execute(
                "INSERT INTO temp.batch_mapping VALUES (?1,?2)",
                [id, database.last_insert_rowid()],
            )
            .map_err(error)?;
    }
    database.execute_batch("INSERT INTO main.reminder_receipts(task_id,token,due_at)
        SELECT source.task_id, CASE WHEN source.token LIKE 'snooze:%' THEN 'snooze:' || mapping.target ELSE source.token END, source.due_at
        FROM incoming.reminder_receipts source
        LEFT JOIN temp.batch_mapping mapping ON mapping.source=CAST(substr(source.token, 8) AS INTEGER) AND source.token LIKE 'snooze:%'
        WHERE source.task_id IN (SELECT id FROM added_tasks) AND (source.token NOT LIKE 'snooze:%' OR mapping.target IS NOT NULL);").map_err(error)?;
    Ok(())
}
