use std::sync::Mutex;

use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::{integration, storage, tasks};

use super::{db, Result, ACTIVE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Complete,
    Snooze,
    Open,
}

impl Action {
    #[cfg(any(windows, test))]
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Snooze => "snooze",
            Self::Open => "open",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "complete" => Some(Self::Complete),
            "snooze" => Some(Self::Snooze),
            "open" => Some(Self::Open),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiAction {
    Snooze { id: i64 },
    Open { id: String },
}

#[derive(Default)]
pub struct UiQueue(Mutex<Vec<UiAction>>);

#[tauri::command]
pub fn pending_notification_actions(state: tauri::State<'_, UiQueue>) -> Vec<UiAction> {
    std::mem::take(&mut *state.0.lock().unwrap())
}

pub(super) fn present(app: &AppHandle, action: UiAction) {
    let state = app.state::<UiQueue>();
    state.0.lock().unwrap().push(action);
    if let Err(error) = integration::show(app, None) {
        eprintln!("Restore Kairos from notification: {error}");
    }
    let _ = app.emit("kairos-notification-action", ());
}

#[cfg(any(windows, test))]
pub(super) struct Link {
    pub scheme: String,
    pub token: String,
}

#[cfg(any(windows, test))]
impl Link {
    pub fn uri(&self, action: Action) -> String {
        format!(
            "{}://activate/{}/{}",
            self.scheme,
            action.name(),
            self.token
        )
    }
}

#[cfg(any(windows, test))]
pub(super) fn scheme(identifier: &str) -> String {
    format!("kairos-notify-{}", identifier.to_ascii_lowercase())
}

#[cfg(any(windows, test))]
fn parse_link<'a>(prefix: &str, arg: &'a str) -> Option<(Action, &'a str)> {
    let path = arg.strip_prefix(prefix)?.strip_prefix("activate/")?;
    let (action, token) = path.split_once('/')?;
    let action = Action::parse(action)?;
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((action, token))
}

pub(super) fn issue(database: &Connection, batch_id: i64, now: i64) -> Result<String> {
    database
        .execute(
            "DELETE FROM reminder_activation_tokens WHERE expires_at<?1",
            [now],
        )
        .map_err(db)?;
    let token = Uuid::new_v4().simple().to_string();
    database
        .execute(
            "INSERT INTO reminder_activation_tokens(batch_id,token,expires_at) VALUES (?1,?2,?3)",
            params![batch_id, token, now + 3 * 24 * 60 * 60],
        )
        .map_err(db)?;
    Ok(token)
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Completed,
    Show(UiAction),
}

fn redeem(
    database: &mut Connection,
    action: Action,
    token: &str,
    now: chrono::DateTime<Local>,
) -> Result<Option<Outcome>> {
    let tx = database
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db)?;
    let record: Option<(i64, String)> = tx
        .query_row(
            &format!(
                "SELECT b.id,t.id FROM reminder_activation_tokens a
        JOIN reminder_batches b ON b.id=a.batch_id JOIN tasks t ON t.id=b.task_id
        WHERE a.token=?1 AND a.expires_at>=?2 AND b.dismissed=0 AND {ACTIVE}"
            ),
            params![token, now.timestamp()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(db)?;
    let Some((id, task)) = record else {
        return Ok(None);
    };
    tx.execute(
        "DELETE FROM reminder_activation_tokens WHERE batch_id=?1",
        [id],
    )
    .map_err(db)?;
    let result = match action {
        Action::Complete => {
            tasks::complete_from_reminder(&tx, &task, now.date_naive())?;
            tx.execute("UPDATE reminder_batches SET dismissed=1 WHERE id=?1", [id])
                .map_err(db)?;
            Outcome::Completed
        }
        Action::Open => Outcome::Show(UiAction::Open { id: task }),
        Action::Snooze => Outcome::Show(UiAction::Snooze { id }),
    };
    tx.commit().map_err(db)?;
    Ok(Some(result))
}

#[cfg(windows)]
pub fn handle_launch(app: &AppHandle, args: &[String]) -> bool {
    let prefix = format!("{}://", scheme(&app.config().identifier));
    let Some(arg) = args.get(1).filter(|arg| arg.starts_with(&prefix)) else {
        return false;
    };
    let Some((action, token)) = parse_link(&prefix, arg) else {
        return true;
    };
    dispatch(app, action, token);
    true
}

pub(super) fn dispatch(app: &AppHandle, action: Action, token: &str) {
    let token = token.to_owned();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let directory = app.path().app_data_dir().map_err(|err| err.to_string())?;
            let mut database = storage::open(&directory).map_err(|err| err.to_string())?;
            redeem(&mut database, action, &token, Local::now())
        })();
        match result {
            Ok(Some(Outcome::Completed)) => {
                let _ = app.emit("kairos-reminders", ());
            }
            Ok(Some(Outcome::Show(action))) => present(&app, action),
            Ok(None) => (),
            Err(error) => eprintln!("Kairos notification activation: {error}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_links_check_the_scheme_action_and_token() {
        let link = Link {
            scheme: scheme("app.Kairos.test"),
            token: "a".repeat(32),
        };
        let prefix = "kairos-notify-app.kairos.test://";
        assert_eq!(
            parse_link(prefix, &link.uri(Action::Snooze)),
            Some((Action::Snooze, link.token.as_str()))
        );
        assert!(parse_link(prefix, &link.uri(Action::Open).replace(".test", ".other")).is_none());
        assert!(parse_link(
            prefix,
            &format!("{}?override=complete", link.uri(Action::Open))
        )
        .is_none());
    }

    #[test]
    fn only_unexpired_tokens_for_active_tasks_are_redeemed_once() {
        let directory = tempfile::tempdir().unwrap();
        let mut database = storage::open(directory.path()).unwrap();
        let now = Local::now();
        for (id, task_id) in [(1, "one"), (2, "two"), (3, "three")] {
            database
                .execute("INSERT INTO tasks(id,title) VALUES (?1,?1)", [task_id])
                .unwrap();
            database.execute("INSERT INTO reminder_batches(id,task_id,created_at,first_due_at,count,silent) VALUES (?1,?2,1,1,1,1)", params![id,task_id]).unwrap();
        }
        let expired = issue(&database, 3, now.timestamp() - 4 * 24 * 60 * 60).unwrap();
        assert_eq!(
            redeem(&mut database, Action::Complete, &expired, now).unwrap(),
            None
        );
        let snooze = issue(&database, 1, now.timestamp()).unwrap();
        assert_eq!(
            redeem(&mut database, Action::Snooze, &snooze, now).unwrap(),
            Some(Outcome::Show(UiAction::Snooze { id: 1 }))
        );
        assert_eq!(
            redeem(&mut database, Action::Complete, &snooze, now).unwrap(),
            None
        );
        let complete = issue(&database, 2, now.timestamp()).unwrap();
        assert_eq!(
            redeem(&mut database, Action::Complete, &complete, now).unwrap(),
            Some(Outcome::Completed)
        );
        assert!(database
            .query_row(
                "SELECT completed_at IS NOT NULL FROM tasks WHERE id='two'",
                [],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        assert!(database
            .query_row(
                "SELECT dismissed FROM reminder_batches WHERE id=2",
                [],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
    }
}
