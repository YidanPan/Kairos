use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate};
use rusqlite::{OptionalExtension, Transaction};
use serde::Serialize;

use super::{db_error, recurrence::date, BoardResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Statistics {
    completed_today: usize,
    week: Vec<Day>,
    weekly_total: usize,
    current_streak: usize,
    best_streak: usize,
    rest_days: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct Day {
    date: String,
    completed: usize,
}

pub(super) fn set_rest_days(tx: &Transaction<'_>, mut days: Vec<u8>) -> BoardResult<()> {
    if days.iter().any(|&day| day > 6) {
        return Err("Choose weekdays from Monday through Sunday.".into());
    }
    days.sort_unstable();
    days.dedup();
    tx.execute(
        "INSERT INTO app_settings(key,value) VALUES ('rest-day-weekdays',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [serde_json::to_string(&days).map_err(|error| error.to_string())?],
    )
    .map_err(db_error)?;
    Ok(())
}

fn local_date(timestamp: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|time| time.with_timezone(&Local).date_naive())
}

#[derive(Default)]
struct PlannedDay {
    required: HashSet<String>,
    skipped: bool,
}

pub(super) fn read(tx: &Transaction<'_>, today: NaiveDate) -> rusqlite::Result<Statistics> {
    let rest_days: Option<String> = tx
        .query_row(
            "SELECT value FROM app_settings WHERE key='rest-day-weekdays'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let rest_days: Vec<u8> = rest_days
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let first_task: Option<String> = tx
        .query_row(
            "SELECT value FROM app_settings WHERE key='statistics-started-at'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let mut start = first_task
        .as_deref()
        .and_then(local_date)
        .unwrap_or(today)
        .min(today);

    let mut completions = HashMap::<NaiveDate, HashSet<String>>::new();
    let mut first_completion = HashMap::<String, NaiveDate>::new();
    let mut completion_query = tx.prepare(
        "SELECT coalesce(task_key, task_id, 'completion:' || id), completed_at
         FROM task_completions WHERE restored_at IS NULL",
    )?;
    let mut rows = completion_query.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let timestamp: String = row.get(1)?;
        if let Some(day) = local_date(&timestamp).filter(|&day| day <= today) {
            start = start.min(day);
            completions.entry(day).or_default().insert(key.clone());
            first_completion
                .entry(key)
                .and_modify(|first| *first = (*first).min(day))
                .or_insert(day);
        }
    }

    let mut plan_query = tx.prepare(
        "SELECT id, scheduled_date, skipped_at IS NOT NULL FROM tasks
         WHERE scheduled_date IS NOT NULL AND (trashed_at IS NULL OR completed_at IS NOT NULL
            OR skipped_at IS NOT NULL OR date(trashed_at, 'localtime') > scheduled_date)
         UNION ALL SELECT task_id, previous_date, 0 FROM task_schedule_history WHERE automatic = 1
         UNION ALL SELECT task_key, planned_on, skipped FROM statistics_plan_archive
         UNION ALL SELECT coalesce(task_key, 'completion:' || id), planned_on, 0 FROM task_completions
         WHERE task_id IS NULL AND restored_at IS NULL AND planned_on IS NOT NULL",
    )?;
    let mut plans = HashMap::<NaiveDate, PlannedDay>::new();
    let mut rows = plan_query.query([])?;
    while let Some(row) = rows.next()? {
        let calendar_date: String = row.get(1)?;
        if let Some(day) = date(&calendar_date).filter(|&day| day >= start && day <= today) {
            let plan = plans.entry(day).or_default();
            if row.get(2)? {
                plan.skipped = true;
            } else {
                plan.required.insert(row.get(0)?);
            }
        }
    }

    let (mut current_streak, mut best_streak) = (0, 0);
    for day in start.iter_days().take_while(|&day| day <= today) {
        if rest_days.contains(&(day.weekday().num_days_from_monday() as u8)) {
            continue;
        }
        if let Some(plan) = plans.get(&day) {
            if plan.skipped && plan.required.is_empty() {
                continue;
            }
            if plan
                .required
                .iter()
                .any(|key| !first_completion.get(key).is_some_and(|&end| end <= day))
            {
                // An unfinished day can still succeed until local midnight.
                if day < today {
                    current_streak = 0;
                }
                continue;
            }
        }
        current_streak += 1;
        best_streak = best_streak.max(current_streak);
    }

    let count = |day: &NaiveDate| completions.get(day).map_or(0, HashSet::len);
    let monday = today - Days::new(u64::from(today.weekday().num_days_from_monday()));
    let week: Vec<_> = monday
        .iter_days()
        .take(7)
        .map(|day| Day {
            date: day.to_string(),
            completed: count(&day),
        })
        .collect();
    let weekly_total = week.iter().map(|day| day.completed).sum();
    Ok(Statistics {
        completed_today: count(&today),
        week,
        weekly_total,
        current_streak,
        best_streak,
        rest_days,
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use rusqlite::params;

    use super::*;

    fn stamp(day: &str) -> String {
        Local
            .from_local_datetime(&date(day).unwrap().and_hms_opt(12, 0, 0).unwrap())
            .single()
            .unwrap()
            .to_utc()
            .to_rfc3339()
    }

    fn task(tx: &Transaction<'_>, id: &str, scheduled: &str, skipped: bool) {
        let skipped_at = skipped.then(|| stamp(scheduled));
        tx.execute("INSERT INTO tasks(id,title,created_at,scheduled_date,skipped_at) VALUES (?1,?1,?2,?3,?4)",
            params![id, stamp("2026-11-16"), scheduled, skipped_at]).unwrap();
    }

    fn finish(tx: &Transaction<'_>, id: &str, day: &str) {
        tx.execute(
            "UPDATE tasks SET completed_at=?2 WHERE id=?1",
            params![id, stamp(day)],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO task_completions(task_id,task_key,completed_at,planned_on)
                    SELECT id,id,completed_at,scheduled_date FROM tasks WHERE id=?1",
            [id],
        )
        .unwrap();
    }

    #[test]
    fn streaks_use_local_plan_dates_and_neutral_days_without_changing_counts() {
        let folder = tempfile::tempdir().unwrap();
        let mut connection = crate::storage::open(folder.path()).unwrap();
        let tx = connection.transaction().unwrap();
        task(&tx, "done on Monday", "2026-11-16", false);
        finish(&tx, "done on Monday", "2026-11-16");
        task(&tx, "skipped on Tuesday", "2026-11-17", true);
        task(&tx, "carried to Thursday", "2026-11-19", false);
        tx.execute(
            "INSERT INTO task_schedule_history(task_id,previous_date,next_date,automatic)
                    VALUES ('carried to Thursday','2026-11-18','2026-11-19',1)",
            [],
        )
        .unwrap();
        finish(&tx, "carried to Thursday", "2026-11-19");
        task(&tx, "skipped Friday", "2026-11-20", true);
        task(&tx, "incomplete Friday", "2026-11-20", false);
        task(&tx, "Saturday rest", "2026-11-21", false);

        set_rest_days(&tx, vec![5, 5]).unwrap();
        assert!(set_rest_days(&tx, vec![7]).is_err());
        let today = date("2026-11-22").unwrap();
        let original = read(&tx, today).unwrap();
        assert_eq!(original.rest_days, [5]);
        assert_eq!((original.current_streak, original.best_streak), (1, 1));
        assert_eq!(original.week[0].date, "2026-11-16");
        assert_eq!(original.week[6].date, "2026-11-22");
        assert_eq!(
            original
                .week
                .iter()
                .map(|day| day.completed)
                .collect::<Vec<_>>(),
            [1, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!((original.completed_today, original.weekly_total), (0, 2));

        set_rest_days(&tx, vec![5, 4, 2]).unwrap();
        let updated = read(&tx, today).unwrap();
        assert_eq!(
            (
                updated.current_streak,
                updated.best_streak,
                updated.weekly_total
            ),
            (3, 3, 2)
        );
        tx.commit().unwrap();
        let mut reopened = crate::storage::open(folder.path()).unwrap();
        assert_eq!(
            read(&reopened.transaction().unwrap(), today)
                .unwrap()
                .rest_days,
            [2, 4, 5]
        );
    }

    #[test]
    fn purging_tasks_keeps_their_statistics_while_restoring_reopens_the_day() {
        let folder = tempfile::tempdir().unwrap();
        let mut connection = crate::storage::open(folder.path()).unwrap();
        let tx = connection.transaction().unwrap();
        let today = date("2026-11-20").unwrap();
        task(&tx, "finished", "2026-11-20", false);
        finish(&tx, "finished", "2026-11-20");
        assert_eq!(read(&tx, today).unwrap().completed_today, 1);
        tx.execute(
            "UPDATE task_completions SET restored_at=?1 WHERE task_id='finished'",
            [stamp("2026-11-20")],
        )
        .unwrap();
        tx.execute("UPDATE tasks SET completed_at=NULL WHERE id='finished'", [])
            .unwrap();
        let open = read(&tx, today).unwrap();
        assert_eq!((open.completed_today, open.current_streak), (0, 4));
        finish(&tx, "finished", "2026-11-20");
        task(&tx, "missed", "2026-11-18", false);
        tx.execute("UPDATE tasks SET trashed_at=?1", [stamp("2026-11-19")])
            .unwrap();
        let before = read(&tx, today).unwrap();
        tx.execute("DELETE FROM tasks", []).unwrap();
        let after = read(&tx, today).unwrap();
        assert_eq!(
            (
                before.completed_today,
                before.current_streak,
                before.best_streak
            ),
            (1, 2, 2)
        );
        assert_eq!(
            (
                after.completed_today,
                after.current_streak,
                after.best_streak
            ),
            (1, 2, 2)
        );
    }

    #[test]
    fn deleting_an_unscheduled_task_keeps_the_start_of_the_streak() {
        let folder = tempfile::tempdir().unwrap();
        let mut connection = crate::storage::open(folder.path()).unwrap();
        let tx = connection.transaction().unwrap();
        tx.execute(
            "INSERT INTO tasks(id,title,created_at) VALUES ('first','First task',?1)",
            [stamp("2026-11-16")],
        )
        .unwrap();
        tx.execute("DELETE FROM tasks WHERE id='first'", [])
            .unwrap();
        assert_eq!(
            read(&tx, date("2026-11-20").unwrap())
                .unwrap()
                .current_streak,
            5
        );
    }
}
