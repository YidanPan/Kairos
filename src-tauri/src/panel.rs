use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewWindow,
};

use crate::{integration, storage};

type SetupError = Box<dyn std::error::Error>;
const GREETING_KEY: &str = "daily-greeting";

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DailyGreeting {
    date: String,
    text: String,
}

pub fn daily_greeting(
    connection: &Connection,
    today: NaiveDate,
) -> rusqlite::Result<Option<DailyGreeting>> {
    let json: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [GREETING_KEY],
            |row| row.get(0),
        )
        .optional()?;
    let today = today.to_string();
    Ok(json
        .and_then(|json| serde_json::from_str::<DailyGreeting>(&json).ok())
        .filter(|greeting| greeting.date == today))
}

fn write_daily_greeting(
    connection: &Connection,
    today: NaiveDate,
    text: &str,
) -> Result<Option<DailyGreeting>, String> {
    let text = text.trim();
    if text.is_empty() {
        connection
            .execute("DELETE FROM app_settings WHERE key = ?1", [GREETING_KEY])
            .map_err(|error| error.to_string())?;
        return Ok(None);
    }
    let greeting = DailyGreeting {
        date: today.to_string(),
        text: text.to_owned(),
    };
    let value = serde_json::to_string(&greeting).map_err(|error| error.to_string())?;
    connection.execute("INSERT INTO app_settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![GREETING_KEY, value]).map_err(|error| error.to_string())?;
    Ok(Some(greeting))
}

#[tauri::command]
pub async fn save_daily_greeting(
    app: AppHandle,
    text: String,
) -> Result<Option<DailyGreeting>, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open(&data_dir).map_err(|error| error.to_string())?;
        write_daily_greeting(&connection, Local::now().date_naive(), &text)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn setup(app: &mut App) -> Result<(), SetupError> {
    let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let restore = MenuItem::with_id(
        app,
        "restore-interaction",
        "Restore Mouse Interaction",
        true,
        None::<&str>,
    )?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &restore, &exit])?;
    let mut tray = TrayIconBuilder::with_id("kairos")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .tooltip("Kairos");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(|app, event| {
        let result = match event.id().as_ref() {
            "exit" => {
                app.exit(0);
                return;
            }
            "settings" => integration::show(app, Some("kairos-open-settings")),
            "show" | "restore-interaction" => integration::show(app, None),
            _ => return,
        };
        if let Err(error) = result {
            eprintln!("Could not show Kairos: {error}");
        }
    })
    .on_tray_icon_event(|tray, event| {
        if !cfg!(target_os = "macos")
            && matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            )
        {
            if let Err(error) = integration::show(tray.app_handle(), None) {
                eprintln!("Could not show Kairos: {error}");
            }
        }
    })
    .build(app)?;
    Ok(())
}

#[tauri::command]
pub async fn load_panel_preferences(app: AppHandle) -> Result<Option<Value>, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open(&data_dir).map_err(|error| error.to_string())?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'panel'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        value
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_panel_preferences(app: AppHandle, preferences: Value) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open(&data_dir).map_err(|error| error.to_string())?;
        connection.execute(
            "INSERT INTO app_settings(key, value) VALUES ('panel', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [preferences.to_string()],
        ).map_err(|error| error.to_string())?;
        Ok(())
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn set_panel_backdrop(window: WebviewWindow, tint: Option<(u8, u8, u8, u8)>) -> bool {
    #[cfg(windows)]
    {
        if let Some(tint) = tint {
            return window_vibrancy::apply_acrylic(&window, Some(tint)).is_ok();
        }
        let _ = window_vibrancy::clear_acrylic(&window);
    }
    #[cfg(target_os = "macos")]
    {
        use tauri::window::{Effect, EffectState, EffectsBuilder};

        let effects = tint.map(|_| {
            EffectsBuilder::new()
                .effect(Effect::Sidebar)
                .state(EffectState::Active)
                .radius(12.0)
                .build()
        });
        return window.set_effects(effects).is_ok() && tint.is_some();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = (window, tint);
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_greeting_persists_for_its_saved_day_and_can_be_cleared() {
        let directory = tempfile::tempdir().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap();
        let next_day = today.succ_opt().unwrap();
        let connection = storage::open(directory.path()).unwrap();
        let saved = write_daily_greeting(&connection, today, "  给今天留一点空白。  ").unwrap();
        assert_eq!(saved.as_ref().unwrap().text, "给今天留一点空白。");
        drop(connection);

        let connection = storage::open(directory.path()).unwrap();
        assert_eq!(daily_greeting(&connection, today).unwrap(), saved);
        assert_eq!(daily_greeting(&connection, next_day).unwrap(), None);
        assert_eq!(
            write_daily_greeting(&connection, next_day, "  ").unwrap(),
            None
        );
        assert_eq!(daily_greeting(&connection, today).unwrap(), None);
    }
}
