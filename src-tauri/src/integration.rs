use std::sync::Mutex;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::storage;

#[cfg(any(windows, target_os = "macos"))]
mod desktop;
#[cfg(windows)]
mod windows;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum WindowMode {
    Desktop,
    AlwaysOnTop,
    #[default]
    Smart,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum StartupPage {
    #[default]
    Tasks,
    Completed,
    Statistics,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Behavior {
    window_mode: WindowMode,
    startup_page: StartupPage,
    whitelist: Vec<String>,
}

impl Behavior {
    fn for_platform(mut self) -> Self {
        if cfg!(target_os = "macos") && matches!(self.window_mode, WindowMode::Smart) {
            self.window_mode = WindowMode::Desktop;
        }
        self
    }
}

#[derive(Default)]
pub struct IntegrationState {
    preferences: Mutex<Behavior>,
    click_through: Mutex<bool>,
    unavailable_shortcuts: Mutex<Vec<&'static str>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Integration {
    preferences: Behavior,
    click_through: bool,
    startup: Option<bool>,
    unavailable_shortcuts: Vec<&'static str>,
}

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let preferences = load_preferences(app).unwrap_or_else(|error| {
        eprintln!("Could not load Kairos behavior: {error}");
        Behavior::default().for_platform()
    });
    #[cfg(any(windows, target_os = "macos"))]
    desktop::setup(app)?;
    app.manage(IntegrationState {
        preferences: Mutex::new(preferences),
        ..IntegrationState::default()
    });
    #[cfg(windows)]
    windows::setup(app);
    #[cfg(target_os = "macos")]
    {
        let state = app.state::<IntegrationState>();
        let mode = state.preferences.lock().unwrap().window_mode;
        apply_window_mode(app, mode)?;
    }
    Ok(())
}

fn load_preferences(app: &AppHandle) -> Result<Behavior, Box<dyn std::error::Error>> {
    let connection = storage::open(&app.path().app_data_dir()?)?;
    let json: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'behavior'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_else(Behavior::default)
        .for_platform())
}

pub fn reload_preferences(app: &AppHandle) {
    if let Ok(preferences) = load_preferences(app) {
        #[cfg(target_os = "macos")]
        if let Err(error) = apply_window_mode(app, preferences.window_mode) {
            eprintln!("Could not restore Kairos window mode: {error}");
        }
        *app.state::<IntegrationState>().preferences.lock().unwrap() = preferences;
    }
}

#[tauri::command]
pub fn integration_status(app: AppHandle) -> Integration {
    let state = app.state::<IntegrationState>();
    let preferences = state.preferences.lock().unwrap().clone();
    let click_through = *state.click_through.lock().unwrap();
    let unavailable_shortcuts = state.unavailable_shortcuts.lock().unwrap().clone();
    Integration {
        preferences,
        click_through,
        #[cfg(any(windows, target_os = "macos"))]
        startup: desktop::startup_enabled(&app).ok(),
        #[cfg(not(any(windows, target_os = "macos")))]
        startup: None,
        unavailable_shortcuts,
    }
}

#[tauri::command]
pub async fn save_behavior(app: AppHandle, mut preferences: Behavior) -> Result<Behavior, String> {
    preferences = preferences.for_platform();
    preferences.whitelist = normalize_whitelist(preferences.whitelist)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<IntegrationState>();
        let mut current = state
            .preferences
            .lock()
            .map_err(|error| error.to_string())?;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let connection = storage::open(&directory).map_err(|error| error.to_string())?;
        let json = serde_json::to_string(&preferences).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES ('behavior',?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [json],
            )
            .map_err(|error| error.to_string())?;
        *current = preferences.clone();
        #[cfg(target_os = "macos")]
        if let Err(error) = apply_window_mode(&app, preferences.window_mode) {
            eprintln!("Could not apply Kairos window mode: {error}");
        }
        Ok(preferences)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(target_os = "macos")]
fn apply_window_mode(app: &AppHandle, mode: WindowMode) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(matches!(mode, WindowMode::AlwaysOnTop))?;
    }
    Ok(())
}

fn normalize_whitelist(entries: Vec<String>) -> Result<Vec<String>, String> {
    let mut list = Vec::new();
    for entry in entries {
        let entry = entry.trim().to_lowercase();
        if entry.is_empty() {
            continue;
        }
        if !entry.ends_with(".exe")
            || entry.len() <= 4
            || entry.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
            || entry.chars().any(char::is_control)
        {
            return Err("Enter Windows executable filenames ending in .exe.".into());
        }
        if !list.contains(&entry) {
            list.push(entry);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    #[cfg(any(windows, target_os = "macos"))]
    return desktop::set_launch_at_login(&app, enabled);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (app, enabled);
        Err("Launch at login is unavailable.".into())
    }
}

#[tauri::command]
pub fn set_click_through(app: AppHandle, enabled: bool) -> Result<bool, String> {
    if enabled && app.tray_by_id("kairos").is_none() {
        return Err("The system tray is needed to restore mouse interaction.".into());
    }
    let window = app
        .get_webview_window("main")
        .ok_or("Kairos window is unavailable.")?;
    let state = app.state::<IntegrationState>();
    let mut current = state
        .click_through
        .lock()
        .map_err(|error| error.to_string())?;
    if *current != enabled {
        window
            .set_ignore_cursor_events(enabled)
            .map_err(|error| error.to_string())?;
        *current = enabled;
        let _ = window.emit("kairos-click-through", enabled);
    }
    Ok(enabled)
}

pub fn show(app: &AppHandle, event: Option<&'static str>) -> Result<(), String> {
    if app.try_state::<IntegrationState>().is_none() {
        return Ok(());
    }
    set_click_through(app.clone(), false)?;
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|error| error.to_string())?;
        window.unminimize().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        if let Some(event) = event {
            window.emit(event, ()).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
