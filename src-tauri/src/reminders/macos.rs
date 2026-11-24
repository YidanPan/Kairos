use std::{io::Write, path::Path, process::Command};

use chrono::Local;
use rusqlite::Connection;
use tauri::AppHandle;
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

use super::{activation, Notice, Result};

const ACTIONS: &str = "kairos-reminder";
const TOKEN: &str = "kairos-activation-token";

pub(crate) fn setup(app: &AppHandle) -> tauri_plugin_notification::Result<()> {
    app.notification()
        .register_action_types(vec![ActionType::builder(ACTIONS)
            .actions(vec![
                Action::builder("open", "Open Task").build(),
                Action::builder("snooze", "Snooze").build(),
                Action::builder("complete", "Complete").build(),
            ])
            .build()])?;

    let handle = app.clone();
    app.notification().on_action(move |event| {
        let action = if event.action_id() == "tap" {
            Some(activation::Action::Open)
        } else {
            activation::Action::parse(event.action_id())
        };
        if let (Some(action), Some(token)) = (
            action,
            event
                .notification()
                .and_then(|item| item.extra().get(TOKEN))
                .and_then(|value| value.as_str()),
        ) {
            activation::dispatch(&handle, action, token);
        }
    })
}

pub(super) fn show(app: &AppHandle, db: &Connection, notice: Notice, custom: bool) -> Result<()> {
    let token = activation::issue(db, notice.id, Local::now().timestamp())?;
    let body = if notice.count > 1 {
        format!("{} reminders due", notice.count)
    } else {
        "Task reminder".to_owned()
    };
    let mut builder = app
        .notification()
        .builder()
        .title(notice.title)
        .body(body)
        .action_type_id(ACTIONS)
        .extra(TOKEN, token);
    if !notice.silent && !custom {
        builder = builder.sound("default");
    }
    builder.show().map_err(|error| error.to_string())
}

pub(super) fn play(directory: &Path, sound: &[u8]) -> Result<()> {
    let mut file = tempfile::Builder::new()
        .prefix("kairos-reminder-")
        .suffix(".wav")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    file.write_all(sound).map_err(|error| error.to_string())?;
    let mut child = Command::new("/usr/bin/afplay")
        .arg(file.path())
        .spawn()
        .map_err(|error| error.to_string())?;
    std::thread::spawn(move || {
        let _file = file;
        let _ = child.wait();
    });
    Ok(())
}
