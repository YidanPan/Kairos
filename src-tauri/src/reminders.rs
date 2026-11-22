use std::{collections::BTreeMap, path::Path, thread, time::Duration};

use chrono::{
    DateTime, Datelike, Local, LocalResult, NaiveDateTime, NaiveTime, TimeZone, Timelike,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{storage, tasks};

#[cfg(test)]
mod tests;

#[cfg(windows)]
mod windows;

type Result<T> = std::result::Result<T, String>;
const OFFSETS: [u16; 5] = [0, 10, 30, 60, 1440];

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Quiet {
    #[default]
    Off,
    Silent,
    Suppress,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Sound {
    #[default]
    System,
    Silent,
    Custom,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Period {
    days: Vec<u8>,
    start: String,
    end: String,
    mode: Quiet,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    manual_dnd: Quiet,
    periods: Vec<Period>,
    sound: Sound,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Rule {
    Offset { minutes: u16 },
    Custom { at: String },
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskPreferences {
    overrides: bool,
    allow_during_dnd: bool,
    rules: Vec<Rule>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    id: i64,
    task_id: String,
    title: String,
    deadline: Option<String>,
    created_at: i64,
    first_due_at: i64,
    count: u32,
    silent: bool,
    native_status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    preferences: Preferences,
    task: TaskPreferences,
    category_offsets: Vec<u16>,
    notices: Vec<Notice>,
    has_custom_sound: bool,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    SavePreferences {
        preferences: Preferences,
    },
    SaveTask {
        id: String,
        preferences: TaskPreferences,
    },
    SaveCategory {
        id: String,
        offsets: Vec<u16>,
    },
    ImportSound {
        bytes: Vec<u8>,
    },
    PreviewSound,
    Dismiss {
        id: i64,
    },
    Snooze {
        id: i64,
        minutes: Option<u16>,
        until: Option<String>,
    },
}

fn db(error: rusqlite::Error) -> String {
    format!("Reminder storage error ({error})")
}

fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn parse<T: DeserializeOwned + Default>(value: Option<String>) -> Result<T> {
    value
        .map(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
        .unwrap_or_else(|| Ok(T::default()))
}

fn settings(database: &Connection) -> Result<Preferences> {
    parse(
        database
            .query_row(
                "SELECT value FROM app_settings WHERE key='notifications'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?,
    )
}

fn task_settings(database: &Connection, id: &str) -> Result<TaskPreferences> {
    let row: Option<(bool, bool, String)> = database
        .query_row(
            "SELECT overrides,allow_during_dnd,rules FROM task_alert_preferences WHERE task_id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(db)?;
    row.map(|(overrides, allow_during_dnd, rules)| {
        Ok(TaskPreferences {
            overrides,
            allow_during_dnd,
            rules: parse(Some(rules))?,
        })
    })
    .transpose()
    .map(|row| row.unwrap_or_default())
}

fn save_task(database: &Connection, id: &str, value: &TaskPreferences) -> Result<()> {
    if value.rules.len() > 100 {
        return Err("Keep at most 100 reminders per task.".into());
    }
    for rule in &value.rules {
        match rule {
            Rule::Offset { minutes } if OFFSETS.contains(minutes) => (),
            Rule::Custom { at } if local_datetime(at).is_some() => (),
            _ => return Err("Choose a valid reminder time.".into()),
        }
    }
    database.execute("INSERT INTO task_alert_preferences(task_id,overrides,allow_during_dnd,rules) VALUES (?1,?2,?3,?4) ON CONFLICT(task_id) DO UPDATE SET overrides=excluded.overrides,allow_during_dnd=excluded.allow_during_dnd,rules=excluded.rules", params![id,value.overrides,value.allow_during_dnd,json(&value.rules)?]).map_err(db)?;
    Ok(())
}

pub(crate) fn inherit(tx: &Transaction<'_>, source: &str, target: &str) -> Result<()> {
    let mut preferences = task_settings(tx, source)?;
    if preferences.overrides || preferences.allow_during_dnd {
        preferences
            .rules
            .retain(|rule| matches!(rule, Rule::Offset { .. }));
        save_task(tx, target, &preferences)?;
    }
    Ok(())
}

fn category_offsets(database: &Connection, id: &str) -> Result<Vec<u16>> {
    parse(
        database
            .query_row(
                "SELECT offsets FROM category_alert_preferences WHERE category_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?,
    )
}

fn clock(value: &str) -> Option<NaiveTime> {
    (value.len() == 5)
        .then(|| NaiveTime::parse_from_str(value, "%H:%M").ok())
        .flatten()
}

fn local_datetime(value: &str) -> Option<NaiveDateTime> {
    (value.len() == 16)
        .then(|| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M").ok())
        .flatten()
}

fn epoch<T: TimeZone>(zone: &T, local: NaiveDateTime) -> Option<i64> {
    // Skipped wall-clock minutes advance to the first time that the local clock can represent.
    for minutes in 0..=180 {
        let at = local.checked_add_signed(chrono::TimeDelta::minutes(minutes))?;
        match zone.from_local_datetime(&at) {
            LocalResult::Single(date) => return Some(date.timestamp()),
            LocalResult::Ambiguous(first, second) => {
                return Some(first.timestamp().min(second.timestamp()))
            }
            LocalResult::None => (),
        }
    }
    None
}

fn quiet_at(value: &Preferences, now: NaiveDateTime) -> Quiet {
    let day = now.weekday().num_days_from_monday() as u8;
    let minute = now
        .time()
        .with_second(0)
        .and_then(|time| time.with_nanosecond(0))
        .unwrap_or(now.time());
    let mut mode = value.manual_dnd;
    for period in &value.periods {
        let (Some(start), Some(end)) = (clock(&period.start), clock(&period.end)) else {
            continue;
        };
        let starts_on = |day| period.days.is_empty() || period.days.contains(&day);
        let active = if start < end {
            starts_on(day) && minute >= start && minute < end
        } else if start > end {
            (minute >= start && starts_on(day)) || (minute < end && starts_on((day + 6) % 7))
        } else {
            false
        };
        if active {
            mode = mode.max(period.mode);
        }
    }
    mode
}

struct Candidate {
    token: String,
    at: i64,
}

const ACTIVE: &str = "t.completed_at IS NULL AND t.trashed_at IS NULL AND t.skipped_at IS NULL";

fn inbox(database: &Connection) -> Result<Vec<Notice>> {
    let sql = format!("SELECT b.id,b.task_id,t.title,t.due_date || ' ' || coalesce(t.due_time,'23:59'),b.created_at,b.first_due_at,b.count,b.silent,b.native_status FROM reminder_batches b JOIN tasks t ON t.id=b.task_id WHERE b.dismissed=0 AND {ACTIVE} ORDER BY b.id DESC");
    let mut statement = database.prepare(&sql).map_err(db)?;
    let items = statement.query_map([], read_notice).map_err(db)?;
    items.collect::<rusqlite::Result<_>>().map_err(db)
}

fn read_notice(row: &rusqlite::Row<'_>) -> rusqlite::Result<Notice> {
    Ok(Notice {
        id: row.get(0)?,
        task_id: row.get(1)?,
        title: row.get(2)?,
        deadline: row.get(3)?,
        created_at: row.get(4)?,
        first_due_at: row.get(5)?,
        count: row.get(6)?,
        silent: row.get(7)?,
        native_status: row.get(8)?,
    })
}

fn collect_due<T: TimeZone>(database: &mut Connection, now: DateTime<T>) -> Result<Vec<Notice>> {
    let transaction = database
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db)?;
    let preferences = settings(&transaction)?;
    let zone = now.timezone();
    let quiet = quiet_at(&preferences, now.naive_local());
    let now = now.timestamp();
    let mut task_query = transaction.prepare(&format!("SELECT t.id,t.title,t.category_id,t.due_date || ' ' || coalesce(t.due_time,'23:59') FROM tasks t WHERE {ACTIVE} AND (EXISTS (SELECT 1 FROM task_alert_preferences p WHERE p.task_id=t.id) OR EXISTS (SELECT 1 FROM category_alert_preferences p WHERE p.category_id=t.category_id) OR EXISTS (SELECT 1 FROM reminder_batches b WHERE b.task_id=t.id AND b.snoozed_for IS NOT NULL))")).map_err(db)?;
    let tasks = task_query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db)?;
    drop(task_query);
    let mut result = Vec::new();
    let mut category_rules = BTreeMap::new();
    for (id, title, category, deadline) in tasks {
        let task = task_settings(&transaction, &id)?;
        let mode = if task.allow_during_dnd {
            Quiet::Off
        } else {
            quiet
        };
        if mode == Quiet::Suppress {
            continue;
        }
        let rules = if task.overrides {
            task.rules
        } else if let Some(category) = category {
            if !category_rules.contains_key(&category) {
                category_rules.insert(category.clone(), category_offsets(&transaction, &category)?);
            }
            category_rules[&category]
                .iter()
                .map(|minutes| Rule::Offset { minutes: *minutes })
                .collect()
        } else {
            vec![]
        };
        let due = deadline
            .as_deref()
            .and_then(|value| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").ok())
            .and_then(|date| epoch(&zone, date));
        let mut candidates = Vec::new();
        for rule in rules {
            match rule {
                Rule::Offset { minutes } => {
                    if let Some(due) = due {
                        candidates.push(Candidate {
                            token: format!("offset:{minutes}"),
                            at: due - i64::from(minutes) * 60,
                        });
                    }
                }
                Rule::Custom { at } => {
                    if let Some(at) = local_datetime(&at).and_then(|date| epoch(&zone, date)) {
                        candidates.push(Candidate {
                            token: "custom".into(),
                            at,
                        });
                    }
                }
            }
        }
        let mut snooze_query = transaction
            .prepare(
                "SELECT id,snoozed_for FROM reminder_batches WHERE task_id=?1 AND snoozed_for<=?2",
            )
            .map_err(db)?;
        let snoozes = snooze_query
            .query_map(params![id, now], |row| {
                Ok(Candidate {
                    token: format!("snooze:{}", row.get::<_, i64>(0)?),
                    at: row.get(1)?,
                })
            })
            .map_err(db)?;
        candidates.extend(snoozes.collect::<rusqlite::Result<Vec<_>>>().map_err(db)?);
        let mut first = now;
        let mut count = 0;
        for candidate in candidates.into_iter().filter(|item| item.at <= now) {
            if transaction.execute("INSERT OR IGNORE INTO reminder_receipts(task_id,token,due_at) VALUES (?1,?2,?3)", params![id,candidate.token,candidate.at]).map_err(db)? == 1 {
                first = first.min(candidate.at);
                count += 1;
            }
        }
        if count == 0 {
            continue;
        }
        let silent = mode == Quiet::Silent || preferences.sound == Sound::Silent;
        transaction.execute("INSERT INTO reminder_batches(task_id,created_at,first_due_at,count,silent) VALUES (?1,?2,?3,?4,?5)",params![id,now,first,count,silent]).map_err(db)?;
        result.push(Notice {
            id: transaction.last_insert_rowid(),
            task_id: id,
            title,
            deadline,
            created_at: now,
            first_due_at: first,
            count,
            silent,
            native_status: "pending".into(),
        });
    }
    transaction.commit().map_err(db)?;
    Ok(result)
}

fn apply(database: &Connection, directory: &Path, action: Action) -> Result<()> {
    match action {
        Action::SavePreferences { preferences } => {
            for period in &preferences.periods {
                let start = clock(&period.start);
                let end = clock(&period.end);
                if start.is_none()
                    || end.is_none()
                    || start == end
                    || period.days.iter().any(|day| *day > 6)
                    || period.mode == Quiet::Off
                {
                    return Err(
                        "Choose a valid quiet period with different start and end times.".into(),
                    );
                }
            }
            if preferences.sound == Sound::Custom && !has_sound(database)? {
                return Err("Choose a WAV file first.".into());
            }
            database.execute("INSERT INTO app_settings(key,value) VALUES ('notifications',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json(&preferences)?]).map_err(db)?;
        }
        Action::SaveTask { id, preferences } => save_task(database, &id, &preferences)?,
        Action::SaveCategory { id, mut offsets } => {
            if offsets.iter().any(|offset| !OFFSETS.contains(offset)) {
                return Err("Choose one of the listed reminder offsets.".into());
            }
            offsets.sort_unstable();
            offsets.dedup();
            database.execute("INSERT INTO category_alert_preferences(category_id,offsets) VALUES (?1,?2) ON CONFLICT(category_id) DO UPDATE SET offsets=excluded.offsets",params![id,json(&offsets)?]).map_err(db)?;
        }
        Action::ImportSound { bytes } => {
            if bytes.len() < 44
                || bytes.len() > 12_000_000
                || &bytes[..4] != b"RIFF"
                || &bytes[8..12] != b"WAVE"
            {
                return Err("Choose a WAV file smaller than 12 MB.".into());
            }
            database
                .execute(
                    "INSERT INTO app_assets(name,contents) VALUES (?1,?2)
                ON CONFLICT(name) DO UPDATE SET contents=excluded.contents",
                    params![storage::NOTIFICATION_SOUND, bytes],
                )
                .map_err(db)?;
        }
        Action::PreviewSound => audio(database, directory)?,
        Action::Dismiss { id } => {
            database
                .execute("UPDATE reminder_batches SET dismissed=1 WHERE id=?1", [id])
                .map_err(db)?;
        }
        Action::Snooze { id, minutes, until } => {
            let now = Local::now();
            let at = match (minutes, until) {
                (Some(minutes @ (5 | 15 | 30 | 60)), None) => {
                    Some(now.timestamp() + i64::from(minutes) * 60)
                }
                (None, Some(local)) => local_datetime(&local).and_then(|date| epoch(&Local, date)),
                _ => None,
            }
            .filter(|time| *time > now.timestamp())
            .ok_or_else(|| "Choose a future snooze time.".to_owned())?;
            database.execute("UPDATE reminder_batches SET dismissed=1,snoozed_for=?2 WHERE id=?1 AND snoozed_for IS NULL",params![id,at]).map_err(db)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn reminder_state(
    app: AppHandle,
    task_id: Option<String>,
    category_id: Option<String>,
    action: Option<Action>,
) -> Result<State> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let database = storage::open(&directory).map_err(|error| error.to_string())?;
        if let Some(action) = action {
            apply(&database, &directory, action)?;
        }
        let task = task_id
            .map(|id| task_settings(&database, &id))
            .transpose()?
            .unwrap_or_default();
        let category_offsets = category_id
            .map(|id| category_offsets(&database, &id))
            .transpose()?
            .unwrap_or_default();
        Ok(State {
            preferences: settings(&database)?,
            task,
            category_offsets,
            notices: inbox(&database)?,
            has_custom_sound: has_sound(&database)?,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn has_sound(database: &Connection) -> Result<bool> {
    database
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM app_assets WHERE name=?1)",
            [storage::NOTIFICATION_SOUND],
            |row| row.get(0),
        )
        .map_err(db)
}

fn audio(database: &Connection, directory: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let sound: Vec<u8> = database
            .query_row(
                "SELECT contents FROM app_assets WHERE name=?1",
                [storage::NOTIFICATION_SOUND],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?
            .ok_or("Choose a WAV file first.")?;
        windows::play(directory, &sound)
    }
    #[cfg(not(windows))]
    {
        let _ = (database, directory);
        Err("WAV preview is available on Windows.".into())
    }
}

#[cfg(windows)]
fn activated(app: AppHandle, notice: Notice, action: &str) {
    match action {
        "complete" => {
            tauri::async_runtime::spawn(async move {
                if tasks::task_board(
                    app.clone(),
                    Some(tasks::Action::SetCompleted {
                        id: notice.task_id,
                        completed: true,
                    }),
                )
                .await
                .is_ok()
                {
                    let _ = app.emit("kairos-reminders", ());
                }
            });
        }
        "snooze" | "open" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if action == "snooze" {
                let _ = app.emit("kairos-snooze-reminder", notice.id);
            } else {
                let _ = app.emit("kairos-open-task", notice.task_id);
            }
        }
        _ => (),
    }
}

pub fn identify(app: &AppHandle) {
    #[cfg(windows)]
    if let Err(error) = windows::set_identity(&app.config().identifier) {
        eprintln!("Kairos process identity: {error}");
    }
    #[cfg(not(windows))]
    let _ = app;
}

pub fn invalidate_callbacks() {
    #[cfg(windows)]
    windows::invalidate_callbacks();
}

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        let Ok(directory) = app.path().app_data_dir() else {
            return;
        };
        #[cfg(windows)]
        let mut toasts = windows::Toasts::new();
        loop {
            let result = storage::open(&directory).map_err(|error| error.to_string()).and_then(|mut database| {
                let notices=collect_due(&mut database, Local::now())?;
                if notices.is_empty() { return Ok(()); }
                let preferences=settings(&database)?;
                for notice in notices {
                    let active=database.query_row(&format!("SELECT EXISTS(SELECT 1 FROM tasks t WHERE t.id=?1 AND {ACTIVE})"),[&notice.task_id],|row| row.get::<_,bool>(0)).map_err(db)?;
                    if !active { continue; }
                    let custom = !notice.silent && preferences.sound==Sound::Custom;
                    let id=notice.id;
                    #[cfg(windows)]
                    let accepted=match toasts.show(&app,notice,custom) {
                        Ok(())=>true,
                        Err(error)=>{ eprintln!("Kairos notification delivery: {error}"); false }
                    };
                    #[cfg(not(windows))]
                    let accepted=false;
                    database.execute("UPDATE reminder_batches SET native_status=?2 WHERE id=?1",
                        params![id,if accepted {"accepted"} else {"unavailable"}]).map_err(db)?;
                    if custom { let _=audio(&database, &directory); }
                }
                let _=app.emit("kairos-reminders",());
                Ok(())
            });
            if let Err(error) = result {
                eprintln!("Kairos reminders: {error}");
            }
            thread::sleep(Duration::from_secs(10));
        }
    });
}
