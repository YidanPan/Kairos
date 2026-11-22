use std::collections::HashSet;

use chrono::{Local, NaiveDateTime, NaiveTime};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{backup, storage};

mod advanced;
mod recurrence;
mod statistics;
use advanced::{Scope, Subtask};
use recurrence::Rule;

#[cfg(test)]
mod advanced_tests;
#[cfg(test)]
mod tests;

type BoardResult<T> = Result<T, String>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    id: String,
    title: String,
    category_id: Option<String>,
    priority: String,
    scheduled_date: Option<String>,
    due_date: Option<String>,
    due_time: Option<String>,
    notes: String,
    created_at: String,
    completed_at: Option<String>,
    trashed_at: Option<String>,
    rescheduled_on: Option<String>,
    manual_order: i64,
    series_id: Option<String>,
    origin_series_id: Option<String>,
    occurrence_on: Option<String>,
    skipped_at: Option<String>,
    recurrence: Option<Rule>,
}

#[derive(Debug, Serialize)]
struct Category {
    id: String,
    name: String,
    color: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleChange {
    task_id: String,
    previous_date: String,
    next_date: Option<String>,
    changed_at: String,
    automatic: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    tasks: Vec<Task>,
    categories: Vec<Category>,
    schedule_history: Vec<ScheduleChange>,
    sort_mode: SortMode,
    subtasks: Vec<Subtask>,
    statistics: statistics::Statistics,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SortMode {
    Automatic,
    Manual,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "camelCase")]
pub enum Change {
    Title(String),
    Category(Option<String>),
    Priority(Priority),
    ScheduledDate(Option<String>),
    Deadline(Deadline),
    Notes(String),
}

#[derive(Clone, Debug, Deserialize)]
pub struct Deadline {
    date: Option<String>,
    time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    Create {
        id: String,
        title: String,
    },
    Edit {
        id: String,
        change: Change,
    },
    SetCompleted {
        id: String,
        completed: bool,
    },
    SaveCategory {
        id: String,
        name: String,
        color: String,
    },
    DeleteCategory {
        id: String,
        replacement: Option<String>,
    },
    SetSort {
        mode: SortMode,
    },
    SetRestDays {
        days: Vec<u8>,
    },
    Reorder {
        ids: Vec<String>,
    },
    EditOccurrence {
        id: String,
        change: Change,
        scope: Scope,
    },
    SaveRecurrence {
        id: String,
        rule: Option<Rule>,
        scope: Scope,
    },
    SkipOccurrence {
        id: String,
    },
    TrashTask {
        id: String,
        scope: Scope,
    },
    TrashCompleted {
        ids: Option<Vec<String>>,
    },
    RestoreTrash {
        ids: Vec<String>,
    },
    PurgeTrash {
        ids: Option<Vec<String>>,
    },
    SaveSubtask {
        task_id: String,
        id: String,
        title: String,
    },
    SetSubtaskCompleted {
        task_id: String,
        id: String,
        completed: bool,
    },
    RemoveSubtask {
        task_id: String,
        id: String,
    },
    MoveSubtask {
        task_id: String,
        id: String,
        neighbor_id: String,
    },
}

#[tauri::command]
pub async fn task_board(app: AppHandle, action: Option<Action>) -> BoardResult<Board> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut database = storage::open(&directory).map_err(|error| error.to_string())?;
        if matches!(
            &action,
            Some(Action::PurgeTrash { .. } | Action::DeleteCategory { .. })
        ) {
            backup::before_change(&directory)?;
        }
        board(&mut database, action, Local::now().naive_local())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn complete_from_reminder(
    transaction: &Transaction<'_>,
    id: &str,
    today: chrono::NaiveDate,
) -> BoardResult<()> {
    advanced::set_completed(transaction, id, true, today)
}

fn board(
    connection: &mut Connection,
    action: Option<Action>,
    now: NaiveDateTime,
) -> BoardResult<Board> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    if let Some(action) = action {
        apply(&transaction, action, now)?;
    }
    carry_forward(&transaction, now).map_err(db_error)?;
    let result = snapshot(&transaction, now.date()).map_err(db_error)?;
    transaction.commit().map_err(db_error)?;
    Ok(result)
}

fn db_error(error: rusqlite::Error) -> String {
    format!("Could not save or load tasks ({error})")
}

fn required_text<'a>(value: &'a str, label: &str) -> BoardResult<&'a str> {
    let text = value.trim();
    if text.is_empty() {
        return Err(format!("Enter a {label}."));
    }
    Ok(text)
}

fn validate_date(value: Option<&str>) -> BoardResult<()> {
    if let Some(value) = value {
        if recurrence::date(value).is_none() {
            return Err("Enter a valid date.".into());
        }
    }
    Ok(())
}

fn apply(tx: &Transaction<'_>, action: Action, now: NaiveDateTime) -> BoardResult<()> {
    match action {
        Action::SetRestDays { days } => statistics::set_rest_days(tx, days)?,
        Action::Create { id, title } => {
            let title = required_text(&title, "task title")?;
            tx.execute(
                "INSERT INTO tasks(id,title,scheduled_date,manual_order) VALUES (?1,?2,?3,(SELECT coalesce(min(manual_order),0)-1 FROM tasks)) ON CONFLICT(id) DO NOTHING",
                params![id, title, now.date().to_string()],
            ).map_err(db_error)?;
        }
        Action::Edit { id, change } => advanced::edit_occurrence(tx, &id, change, Scope::This)?,
        Action::SetCompleted { id, completed } => {
            advanced::set_completed(tx, &id, completed, now.date())?
        }
        Action::SaveCategory { id, name, color } => {
            let name = required_text(&name, "category name")?;
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("Choose a valid category color.".into());
            }
            if tx.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE name=?1 COLLATE NOCASE AND id<>?2)", params![name,id], |row| row.get::<_, bool>(0)).map_err(db_error)? {
                return Err("A category with this name already exists.".into());
            }
            tx.execute(
                "INSERT INTO categories(id,name,color) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color",
                params![id, name, color],
            ).map_err(db_error)?;
        }
        Action::DeleteCategory { id, replacement } => {
            if let Some(replacement) = replacement {
                if replacement == id {
                    return Err("Choose a different category.".into());
                }
                tx.execute(
                    "UPDATE tasks SET category_id=?2 WHERE category_id=?1",
                    params![id, replacement],
                )
                .map_err(db_error)?;
                tx.execute(
                    "UPDATE recurrence_series SET category_id=?2 WHERE category_id=?1",
                    params![id, replacement],
                )
                .map_err(db_error)?;
            }
            tx.execute("DELETE FROM categories WHERE id=?1", [id])
                .map_err(db_error)?;
        }
        Action::SetSort { mode } => {
            let value = match mode {
                SortMode::Automatic => "automatic",
                SortMode::Manual => "manual",
            };
            tx.execute(
                "INSERT INTO app_settings(key,value) VALUES ('task-sort',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [value],
            ).map_err(db_error)?;
        }
        Action::Reorder { ids } => reorder(tx, &ids)?,
        other => advanced::apply(tx, other, now.date())?,
    }
    Ok(())
}

fn edit(tx: &Transaction<'_>, id: &str, change: Change) -> BoardResult<()> {
    let (column, value) = match change {
        Change::Title(value) => (
            "title",
            Some(required_text(&value, "task title")?.to_owned()),
        ),
        Change::Category(value) => ("category_id", value),
        Change::Priority(priority) => ("priority", Some(priority.as_str().into())),
        Change::Notes(value) => ("notes", Some(value)),
        Change::ScheduledDate(value) => {
            validate_date(value.as_deref())?;
            tx.execute(
                "INSERT INTO task_schedule_history(task_id,previous_date,next_date,automatic) SELECT id,scheduled_date,?2,0 FROM tasks WHERE id=?1 AND scheduled_date IS NOT NULL AND scheduled_date IS NOT ?2",
                params![id, value],
            ).map_err(db_error)?;
            tx.execute(
                "UPDATE tasks SET scheduled_date=?2,rescheduled_on=NULL WHERE id=?1",
                params![id, value],
            )
            .map_err(db_error)?;
            return Ok(());
        }
        Change::Deadline(Deadline { date, time }) => {
            validate_date(date.as_deref())?;
            if let Some(time) = &time {
                if date.is_none()
                    || time.len() != 5
                    || NaiveTime::parse_from_str(time, "%H:%M").is_err()
                {
                    return Err("Choose a due date and a valid due time.".into());
                }
            }
            tx.execute(
                "UPDATE tasks SET due_date=?2,due_time=?3 WHERE id=?1",
                params![id, date, time],
            )
            .map_err(db_error)?;
            return Ok(());
        }
    };
    let count = tx
        .execute(
            &format!("UPDATE tasks SET {column}=?2 WHERE id=?1"),
            params![id, value],
        )
        .map_err(db_error)?;
    if count == 0 {
        return Err("This task is unavailable.".into());
    }
    Ok(())
}

fn reorder(tx: &Transaction<'_>, ids: &[String]) -> BoardResult<()> {
    let selected: HashSet<_> = ids.iter().map(String::as_str).collect();
    if selected.len() != ids.len() {
        return Err("A task appears more than once in the sort order.".into());
    }
    let mut query = tx.prepare("SELECT id FROM tasks WHERE completed_at IS NULL AND trashed_at IS NULL AND skipped_at IS NULL ORDER BY manual_order, created_at DESC, id").map_err(db_error)?;
    let active: Vec<String> = query
        .query_map([], |row| row.get(0))
        .map_err(db_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(db_error)?;
    if active
        .iter()
        .filter(|id| selected.contains(id.as_str()))
        .count()
        != ids.len()
    {
        return Err("Some tasks changed while sorting. Try again.".into());
    }
    let mut next = ids.iter();
    let mut update = tx
        .prepare("UPDATE tasks SET manual_order=?2 WHERE id=?1 AND manual_order<>?2")
        .map_err(db_error)?;
    for (index, id) in active.iter().enumerate() {
        let id = if selected.contains(id.as_str()) {
            next.next().unwrap_or(id)
        } else {
            id
        };
        update
            .execute(params![id, index as i64])
            .map_err(db_error)?;
    }
    Ok(())
}

fn carry_forward(tx: &Transaction<'_>, now: NaiveDateTime) -> rusqlite::Result<()> {
    let today = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M:%S").to_string();
    let candidates = "completed_at IS NULL AND trashed_at IS NULL AND skipped_at IS NULL AND scheduled_date < ?1
        AND (due_date IS NULL OR due_date > ?1 OR (due_date = ?1 AND coalesce(due_time,'23:59') || ':00' > ?2))";
    tx.execute(
        &format!("INSERT INTO task_schedule_history(task_id,previous_date,next_date,automatic) SELECT id,scheduled_date,?1,1 FROM tasks WHERE {candidates}"),
        params![today,time],
    )?;
    tx.execute(
        &format!("UPDATE tasks SET scheduled_date=?1,rescheduled_on=?1 WHERE {candidates}"),
        params![today, time],
    )?;
    Ok(())
}

fn snapshot(tx: &Transaction<'_>, today: chrono::NaiveDate) -> rusqlite::Result<Board> {
    let mut task_query = tx.prepare("SELECT t.id,t.title,t.category_id,t.priority,t.scheduled_date,t.due_date,t.due_time,t.notes,t.created_at,t.completed_at,t.trashed_at,t.rescheduled_on,t.manual_order,t.series_id,t.origin_series_id,t.occurrence_on,t.skipped_at,s.rule
        FROM tasks t LEFT JOIN recurrence_series s ON s.id=t.series_id ORDER BY t.manual_order,t.created_at DESC,t.id")?;
    let tasks = task_query
        .query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                category_id: row.get(2)?,
                priority: row.get(3)?,
                scheduled_date: row.get(4)?,
                due_date: row.get(5)?,
                due_time: row.get(6)?,
                notes: row.get(7)?,
                created_at: row.get(8)?,
                completed_at: row.get(9)?,
                trashed_at: row.get(10)?,
                rescheduled_on: row.get(11)?,
                manual_order: row.get(12)?,
                series_id: row.get(13)?,
                origin_series_id: row.get(14)?,
                occurrence_on: row.get(15)?,
                skipped_at: row.get(16)?,
                recurrence: advanced::read_rule(row, 17)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut category_query =
        tx.prepare("SELECT id,name,color FROM categories ORDER BY name COLLATE NOCASE,id")?;
    let categories = category_query
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut history_query = tx.prepare("SELECT task_id,previous_date,next_date,changed_at,automatic FROM task_schedule_history ORDER BY id DESC")?;
    let schedule_history = history_query
        .query_map([], |row| {
            Ok(ScheduleChange {
                task_id: row.get(0)?,
                previous_date: row.get(1)?,
                next_date: row.get(2)?,
                changed_at: row.get(3)?,
                automatic: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let sort: Option<String> = tx
        .query_row(
            "SELECT value FROM app_settings WHERE key='task-sort'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let sort_mode = if sort.as_deref() == Some("manual") {
        SortMode::Manual
    } else {
        SortMode::Automatic
    };
    Ok(Board {
        tasks,
        categories,
        schedule_history,
        sort_mode,
        subtasks: advanced::subtasks(tx)?,
        statistics: statistics::read(tx, today)?,
    })
}
