use rusqlite::OptionalExtension;
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, WebviewWindow,
};

use crate::{integration, storage};

type SetupError = Box<dyn std::error::Error>;

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
        .show_menu_on_left_click(false)
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
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
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
    #[cfg(not(windows))]
    let _ = (window, tint);
    false
}
