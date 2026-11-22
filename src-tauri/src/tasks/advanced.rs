use chrono::{Datelike, NaiveDate, TimeDelta, Utc};
use rusqlite::{params, types::Type, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};

use super::{
    db_error, edit, recurrence::date, required_text, Action, BoardResult, Change, Deadline, Rule,
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    This,
    Future,
    Entire,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Subtask {
    id: String,
    task_id: String,
    title: String,
    completed_at: Option<String>,
}

struct Occurrence {
    series_id: Option<String>,
    occurrence_on: Option<String>,
    scheduled_date: Option<String>,
    due_date: Option<String>,
    completed: bool,
    trashed: bool,
    skipped: bool,
}

impl Occurrence {
    fn active(&self) -> bool {
        !self.completed && !self.trashed && !self.skipped
    }

    fn require_active(&self) -> BoardResult<()> {
        if !self.active() {
            return Err("Restore the task before changing its subtasks or recurrence.".into());
        }
        Ok(())
    }

    fn scheduled(&self) -> BoardResult<NaiveDate> {
        self.scheduled_date
            .as_deref()
            .and_then(date)
            .ok_or_else(|| "Choose a scheduled date before adding recurrence.".into())
    }

    fn boundary(&self) -> BoardResult<&str> {
        self.occurrence_on
            .as_deref()
            .ok_or_else(|| "The occurrence date is missing.".into())
    }
}

fn occurrence(tx: &Transaction<'_>, id: &str) -> BoardResult<Occurrence> {
    tx.query_row(
        "SELECT series_id,occurrence_on,scheduled_date,due_date,completed_at IS NOT NULL,trashed_at IS NOT NULL,skipped_at IS NOT NULL FROM tasks WHERE id=?1",
        [id],
        |row| Ok(Occurrence { series_id: row.get(0)?, occurrence_on: row.get(1)?, scheduled_date: row.get(2)?, due_date: row.get(3)?, completed: row.get(4)?, trashed: row.get(5)?, skipped: row.get(6)? }),
    ).optional().map_err(db_error)?.ok_or_else(|| "This task is unavailable.".into())
}

pub(super) fn read_rule(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Rule>> {
    row.get::<_, Option<String>>(index)?
        .map(|text| {
            serde_json::from_str(&text).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

pub(super) fn subtasks(tx: &Transaction<'_>) -> rusqlite::Result<Vec<Subtask>> {
    let mut query = tx.prepare(
        "SELECT id,task_id,title,completed_at FROM subtasks ORDER BY task_id,position,id",
    )?;
    let rows = query.query_map([], |row| {
        Ok(Subtask {
            id: row.get(0)?,
            task_id: row.get(1)?,
            title: row.get(2)?,
            completed_at: row.get(3)?,
        })
    })?;
    rows.collect()
}

fn today_stamp() -> String {
    Utc::now().to_rfc3339()
}

fn new_id(tx: &Transaction<'_>) -> BoardResult<String> {
    tx.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(db_error)
}

fn shift_date(base: NaiveDate, days: i64) -> BoardResult<String> {
    TimeDelta::try_days(days)
        .and_then(|offset| base.checked_add_signed(offset))
        .filter(|day| (1..=9999).contains(&day.year()))
        .map(|day| day.to_string())
        .ok_or_else(|| "The resulting deadline is outside the supported date range.".into())
}

fn detach(tx: &Transaction<'_>, id: &str) -> BoardResult<()> {
    tx.execute("UPDATE tasks SET origin_series_id=series_id,series_id=NULL WHERE id=?1 AND series_id IS NOT NULL", [id]).map_err(db_error)?;
    Ok(())
}

fn detach_if_occupied(tx: &Transaction<'_>, id: &str, entry: &Occurrence) -> BoardResult<()> {
    let Some(series) = &entry.series_id else {
        return Ok(());
    };
    let occupied = tx.query_row(
        "SELECT (stopped=1 OR (stop_on IS NOT NULL AND stop_on<=?3) OR EXISTS(
            SELECT 1 FROM tasks WHERE id<>?1 AND series_id=?2 AND completed_at IS NULL AND trashed_at IS NULL AND skipped_at IS NULL
        )) FROM recurrence_series WHERE id=?2",
        params![id, series, entry.boundary()?], |row| row.get::<_, bool>(0),
    ).map_err(db_error)?;
    if occupied {
        detach(tx, id)?;
    }
    Ok(())
}

pub(super) fn set_completed(
    tx: &Transaction<'_>,
    id: &str,
    completed: bool,
    today: NaiveDate,
) -> BoardResult<()> {
    let entry = occurrence(tx, id)?;
    if entry.trashed || entry.skipped || completed == entry.completed {
        return Ok(());
    }
    let stamp = today_stamp();
    if completed {
        tx.execute(
            "UPDATE tasks SET completed_at=?2 WHERE id=?1",
            params![id, stamp],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE subtasks SET completed_at=?2 WHERE task_id=?1 AND completed_at IS NULL",
            params![id, stamp],
        )
        .map_err(db_error)?;
        tx.execute("INSERT INTO task_completions(task_id,task_key,completed_at,planned_on) SELECT id,id,completed_at,scheduled_date FROM tasks WHERE id=?1", [id]).map_err(db_error)?;
        advance(tx, id, &entry, today)?;
    } else {
        detach_if_occupied(tx, id, &entry)?;
        tx.execute(
            "UPDATE task_completions SET restored_at=?2 WHERE task_id=?1 AND restored_at IS NULL",
            params![id, stamp],
        )
        .map_err(db_error)?;
        tx.execute("UPDATE tasks SET completed_at=NULL WHERE id=?1", [id])
            .map_err(db_error)?;
    }
    Ok(())
}

fn advance(
    tx: &Transaction<'_>,
    source: &str,
    entry: &Occurrence,
    today: NaiveDate,
) -> BoardResult<()> {
    let Some(series) = &entry.series_id else {
        return Ok(());
    };
    let values = tx
        .query_row(
            "SELECT anchor_date,rule,due_offset,stop_on,stopped FROM recurrence_series WHERE id=?1",
            [series],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    read_rule(row, 1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            },
        )
        .map_err(db_error)?;
    let (anchor, rule, offset, stop_on, stopped) = values;
    if stopped {
        return Ok(());
    }
    let anchor = date(&anchor).ok_or_else(|| "Invalid recurrence start.".to_owned())?;
    let boundary = date(entry.boundary()?).ok_or_else(|| "Invalid occurrence date.".to_owned())?;
    let Some(first) = today.max(boundary).succ_opt() else {
        return Ok(());
    };
    let rule = rule.ok_or_else(|| "The recurrence rule is missing.".to_owned())?;
    let Some(next) = rule.next_on_or_after(anchor, first) else {
        return Ok(());
    };
    if stop_on
        .as_deref()
        .and_then(date)
        .is_some_and(|end| next >= end)
    {
        return Ok(());
    }
    let due = offset.map(|offset| shift_date(next, offset)).transpose()?;
    let id = new_id(tx)?;
    tx.execute(
        "INSERT INTO tasks(id,title,category_id,priority,notes,scheduled_date,due_date,due_time,series_id,occurrence_on,manual_order)
         SELECT ?1,title,category_id,priority,notes,?2,?3,due_time,id,?2,(SELECT coalesce(min(manual_order),0)-1 FROM tasks)
         FROM recurrence_series WHERE id=?4",
        params![id, next.to_string(), due, series],
    ).map_err(db_error)?;
    tx.execute("INSERT INTO subtasks(id,task_id,title,position) SELECT lower(hex(randomblob(16))),?1,title,position FROM subtasks WHERE task_id=?2", params![id,source]).map_err(db_error)?;
    crate::reminders::inherit(tx, source, &id)?;
    Ok(())
}

fn set_recurrence(
    tx: &Transaction<'_>,
    id: &str,
    rule: Option<Rule>,
    scope: Scope,
    today: NaiveDate,
) -> BoardResult<()> {
    let entry = occurrence(tx, id)?;
    entry.require_active()?;
    let Some(rule) = rule else {
        if let Some(series) = &entry.series_id {
            if scope != Scope::This {
                stop_series(tx, series, &entry, scope)?;
            }
            detach(tx, id)?;
            if scope == Scope::This {
                advance(tx, id, &entry, today)?;
            }
        }
        return Ok(());
    };
    let anchor = entry.scheduled()?;
    let rule_text = serde_json::to_string(&rule).map_err(|error| error.to_string())?;
    if let Some(series) = &entry.series_id {
        let start = match scope {
            Scope::This => return Err("Use Scheduled date to move only this occurrence.".into()),
            Scope::Future => entry.boundary()?.to_owned(),
            Scope::Entire => tx
                .query_row(
                    "SELECT anchor_date FROM recurrence_series WHERE id=?1",
                    [series],
                    |row| row.get(0),
                )
                .map_err(db_error)?,
        };
        rule.validate(date(&start).ok_or_else(|| "Invalid recurrence start.".to_owned())?)?;
        tx.execute("UPDATE recurrence_series SET rule=?2,anchor_date=?3,stopped=0,stop_on=NULL WHERE id=?1", params![series,rule_text,start]).map_err(db_error)?;
    } else {
        rule.validate(anchor)?;
        let series = new_id(tx)?;
        let offset = entry
            .due_date
            .as_deref()
            .and_then(date)
            .map(|due| due.signed_duration_since(anchor).num_days());
        tx.execute(
            "INSERT INTO recurrence_series(id,anchor_date,rule,title,category_id,priority,notes,due_offset,due_time)
             SELECT ?2,scheduled_date,?3,title,category_id,priority,notes,?4,due_time FROM tasks WHERE id=?1",
            params![id,series,rule_text,offset],
        ).map_err(db_error)?;
        tx.execute(
            "UPDATE tasks SET series_id=?2,occurrence_on=scheduled_date WHERE id=?1",
            params![id, series],
        )
        .map_err(db_error)?;
    }
    Ok(())
}

fn stop_series(
    tx: &Transaction<'_>,
    series: &str,
    entry: &Occurrence,
    scope: Scope,
) -> BoardResult<()> {
    match scope {
        Scope::Entire => tx.execute("UPDATE recurrence_series SET stopped=1 WHERE id=?1", [series]),
        Scope::Future => tx.execute("UPDATE recurrence_series SET stop_on=CASE WHEN stop_on IS NULL OR stop_on>?2 THEN ?2 ELSE stop_on END WHERE id=?1", params![series,entry.boundary()?]),
        Scope::This => return Ok(()),
    }.map_err(db_error)?;
    Ok(())
}

fn scope_tasks(
    tx: &Transaction<'_>,
    series: &str,
    entry: &Occurrence,
    scope: Scope,
) -> BoardResult<Vec<String>> {
    let boundary = if scope == Scope::Future {
        Some(entry.boundary()?)
    } else {
        None
    };
    let mut query = tx.prepare("SELECT id FROM tasks WHERE series_id=?1 AND trashed_at IS NULL AND (?2 IS NULL OR occurrence_on>=?2)").map_err(db_error)?;
    let rows = query
        .query_map(params![series, boundary], |row| row.get(0))
        .map_err(db_error)?;
    rows.collect::<rusqlite::Result<_>>().map_err(db_error)
}

fn reanchor(
    tx: &Transaction<'_>,
    series: &str,
    ids: Vec<String>,
    value: Option<String>,
) -> BoardResult<()> {
    let start = value
        .as_deref()
        .and_then(date)
        .ok_or_else(|| "Choose a scheduled date for the recurrence.".to_owned())?;
    let rule = tx
        .query_row(
            "SELECT rule FROM recurrence_series WHERE id=?1",
            [series],
            |row| read_rule(row, 0),
        )
        .map_err(db_error)?;
    rule.ok_or_else(|| "The recurrence rule is missing.".to_owned())?
        .validate(start)?;
    let offset: Option<i64> = tx
        .query_row(
            "SELECT due_offset FROM recurrence_series WHERE id=?1",
            [series],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let due = offset.map(|days| shift_date(start, days)).transpose()?;
    for id in ids {
        if !occurrence(tx, &id)?.active() {
            continue;
        }
        edit(tx, &id, Change::ScheduledDate(value.clone()))?;
        tx.execute("UPDATE tasks SET occurrence_on=?2,due_date=?3,due_time=(SELECT due_time FROM recurrence_series WHERE id=?4) WHERE id=?1", params![id,value,due,series]).map_err(db_error)?;
    }
    tx.execute(
        "UPDATE recurrence_series SET anchor_date=?2 WHERE id=?1",
        params![series, value],
    )
    .map_err(db_error)?;
    Ok(())
}

pub(super) fn edit_occurrence(
    tx: &Transaction<'_>,
    id: &str,
    change: Change,
    scope: Scope,
) -> BoardResult<()> {
    let entry = occurrence(tx, id)?;
    if entry.trashed || entry.skipped {
        return Err("This archived task is read-only.".into());
    }
    let Some(series) = &entry.series_id else {
        return edit(tx, id, change);
    };
    if scope == Scope::This {
        if entry.active() && matches!(change, Change::ScheduledDate(None)) {
            return Err("A recurring task needs a scheduled date.".into());
        }
        return edit(tx, id, change);
    }
    let ids = scope_tasks(tx, series, &entry, scope)?;
    let (column, value) = match &change {
        Change::Title(title) => (
            "title",
            Some(required_text(title, "task title")?.to_owned()),
        ),
        Change::Category(id) => ("category_id", id.clone()),
        Change::Priority(priority) => ("priority", Some(priority.as_str().to_owned())),
        Change::Notes(notes) => ("notes", Some(notes.clone())),
        Change::ScheduledDate(value) => return reanchor(tx, series, ids, value.clone()),
        Change::Deadline(deadline) => {
            // Use the edited occurrence's displayed schedule, then apply the same offset from each task's own schedule.
            edit(tx, id, change.clone())?;
            let start = entry.scheduled()?;
            let offset = deadline
                .date
                .as_deref()
                .and_then(date)
                .map(|due| due.signed_duration_since(start).num_days());
            for other in ids.into_iter().filter(|other| other != id) {
                let affected = occurrence(tx, &other)?;
                let due = offset
                    .map(|days| affected.scheduled().and_then(|base| shift_date(base, days)))
                    .transpose()?;
                edit(
                    tx,
                    &other,
                    Change::Deadline(Deadline {
                        date: due,
                        time: deadline.time.clone(),
                    }),
                )?;
            }
            tx.execute(
                "UPDATE recurrence_series SET due_offset=?2,due_time=?3 WHERE id=?1",
                params![series, offset, deadline.time],
            )
            .map_err(db_error)?;
            return Ok(());
        }
    };
    for affected in ids {
        edit(tx, &affected, change.clone())?;
    }
    tx.execute(
        &format!("UPDATE recurrence_series SET {column}=?2 WHERE id=?1"),
        params![series, value],
    )
    .map_err(db_error)?;
    Ok(())
}

fn trash_task(tx: &Transaction<'_>, id: &str, scope: Scope, today: NaiveDate) -> BoardResult<()> {
    let entry = occurrence(tx, id)?;
    if entry.trashed {
        return Ok(());
    }
    let stamp = today_stamp();
    if let Some(series) = &entry.series_id {
        if scope != Scope::This {
            stop_series(tx, series, &entry, scope)?;
            let cutoff = if scope == Scope::Future {
                Some(entry.boundary()?)
            } else {
                None
            };
            tx.execute("UPDATE tasks SET trashed_at=?2 WHERE series_id=?1 AND completed_at IS NULL AND skipped_at IS NULL AND trashed_at IS NULL AND (?3 IS NULL OR occurrence_on>=?3)", params![series,stamp,cutoff]).map_err(db_error)?;
            return Ok(());
        }
    }
    tx.execute(
        "UPDATE tasks SET trashed_at=?2 WHERE id=?1",
        params![id, stamp],
    )
    .map_err(db_error)?;
    if entry.active() {
        advance(tx, id, &entry, today)?;
    }
    Ok(())
}

pub(super) fn apply(tx: &Transaction<'_>, action: Action, today: NaiveDate) -> BoardResult<()> {
    match action {
        Action::EditOccurrence { id, change, scope } => edit_occurrence(tx, &id, change, scope)?,
        Action::SaveRecurrence { id, rule, scope } => set_recurrence(tx, &id, rule, scope, today)?,
        Action::SkipOccurrence { id } => {
            let entry = occurrence(tx, &id)?;
            if entry.skipped {
                return Ok(());
            }
            if !entry.active() || entry.series_id.is_none() {
                return Err("Only an active recurring task can be skipped.".into());
            }
            tx.execute(
                "UPDATE tasks SET skipped_at=?2 WHERE id=?1",
                params![id, today_stamp()],
            )
            .map_err(db_error)?;
            advance(tx, &id, &entry, today)?;
        }
        Action::TrashTask { id, scope } => trash_task(tx, &id, scope, today)?,
        Action::TrashCompleted { ids } => {
            let stamp = today_stamp();
            if let Some(ids) = ids {
                for id in ids {
                    tx.execute("UPDATE tasks SET trashed_at=?2 WHERE id=?1 AND completed_at IS NOT NULL AND trashed_at IS NULL", params![id,stamp]).map_err(db_error)?;
                }
            } else {
                tx.execute("UPDATE tasks SET trashed_at=?1 WHERE completed_at IS NOT NULL AND trashed_at IS NULL", [stamp]).map_err(db_error)?;
            }
        }
        Action::RestoreTrash { ids } => {
            for id in ids {
                let entry = occurrence(tx, &id)?;
                if entry.trashed && !entry.completed && !entry.skipped {
                    detach_if_occupied(tx, &id, &entry)?;
                }
                tx.execute("UPDATE tasks SET trashed_at=NULL WHERE id=?1", [id])
                    .map_err(db_error)?;
            }
        }
        Action::PurgeTrash { ids } => {
            if let Some(ids) = ids {
                for id in ids {
                    tx.execute(
                        "DELETE FROM tasks WHERE id=?1 AND trashed_at IS NOT NULL",
                        [id],
                    )
                    .map_err(db_error)?;
                }
            } else {
                tx.execute("DELETE FROM tasks WHERE trashed_at IS NOT NULL", [])
                    .map_err(db_error)?;
            }
        }
        Action::SaveSubtask { task_id, id, title } => {
            occurrence(tx, &task_id)?.require_active()?;
            let title = required_text(&title, "subtask title")?;
            tx.execute(
                "INSERT INTO subtasks(id,task_id,title,position) VALUES (?1,?2,?3,(SELECT coalesce(max(position),-1)+1 FROM subtasks WHERE task_id=?2))
                 ON CONFLICT(id) DO UPDATE SET title=excluded.title WHERE task_id=excluded.task_id",
                params![id,task_id,title],
            ).map_err(db_error)?;
        }
        Action::SetSubtaskCompleted {
            task_id,
            id,
            completed,
        } => {
            let entry = occurrence(tx, &task_id)?;
            if completed && entry.completed && !entry.trashed {
                return Ok(());
            }
            entry.require_active()?;
            let count = tx.execute("UPDATE subtasks SET completed_at=CASE WHEN ?3 IS NULL THEN NULL ELSE coalesce(completed_at,?3) END WHERE id=?1 AND task_id=?2", params![id,task_id,completed.then(today_stamp)]).map_err(db_error)?;
            if count == 0 {
                return Err("This subtask is unavailable.".into());
            }
            let remaining = tx
                .query_row(
                    "SELECT count(*) FROM subtasks WHERE task_id=?1 AND completed_at IS NULL",
                    [&task_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(db_error)?;
            if completed && remaining == 0 {
                set_completed(tx, &task_id, true, today)?;
            }
        }
        Action::RemoveSubtask { task_id, id } => {
            occurrence(tx, &task_id)?.require_active()?;
            tx.execute(
                "DELETE FROM subtasks WHERE id=?1 AND task_id=?2",
                params![id, task_id],
            )
            .map_err(db_error)?;
        }
        Action::MoveSubtask {
            task_id,
            id,
            neighbor_id,
        } => {
            occurrence(tx, &task_id)?.require_active()?;
            let slots = tx.query_row("SELECT a.position,b.position FROM subtasks a JOIN subtasks b ON a.task_id=b.task_id WHERE a.id=?1 AND b.id=?2 AND a.task_id=?3", params![id,neighbor_id,task_id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).optional().map_err(db_error)?;
            let (first, second) =
                slots.ok_or_else(|| "The subtask order changed. Try again.".to_owned())?;
            tx.execute("UPDATE subtasks SET position=CASE id WHEN ?1 THEN ?3 ELSE ?4 END WHERE id IN (?1,?2)", params![id,neighbor_id,second,first]).map_err(db_error)?;
        }
        _ => unreachable!("Core events are handled by the board"),
    }
    Ok(())
}
