use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use super::{set_click_through, show, IntegrationState};

const SHORTCUTS: [(&str, &str, Option<&str>); 5] = [
    ("T", "Show or hide", None),
    ("N", "New task", Some("kairos-new-task")),
    ("F", "Search", Some("kairos-search")),
    ("S", "Settings", Some("kairos-open-settings")),
    ("R", "Restore mouse interaction", None),
];

pub(super) fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.plugin(
        tauri_plugin_autostart::Builder::new()
            .app_name(app.config().identifier.clone())
            .build(),
    )?;
    let state = app.state::<IntegrationState>();
    let modifiers = if cfg!(target_os = "macos") {
        "Super+Alt+Shift"
    } else {
        "Ctrl+Alt+Shift"
    };
    for (key, label, event) in SHORTCUTS {
        let binding = format!("{modifiers}+{key}");
        let registration =
            app.global_shortcut()
                .on_shortcut(binding.as_str(), move |app, _, shortcut| {
                    if shortcut.state == ShortcutState::Released {
                        let result = match key {
                            "T" => toggle_window(app),
                            "R" => set_click_through(app.clone(), false).map(|_| ()),
                            _ => show(app, event),
                        };
                        if let Err(error) = result {
                            eprintln!("Kairos shortcut {key}: {error}");
                        }
                    }
                });
        if let Err(error) = registration {
            state.unavailable_shortcuts.lock().unwrap().push(label);
            eprintln!("Could not register Kairos shortcut {binding}: {error}");
        }
    }
    Ok(())
}

pub(super) fn startup_enabled(app: &AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

pub(super) fn set_launch_at_login(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    let startup = app.autolaunch();
    if startup_enabled(app)? != enabled {
        if enabled {
            startup.enable()
        } else {
            startup.disable()
        }
        .map_err(|error| error.to_string())?;
    }
    startup_enabled(app)
}

fn toggle_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Kairos window is unavailable.")?;
    let state = app.state::<IntegrationState>();
    let click_through = *state
        .click_through
        .lock()
        .map_err(|error| error.to_string())?;
    let visible = window.is_visible().map_err(|error| error.to_string())?;
    let minimized = window.is_minimized().map_err(|error| error.to_string())?;
    if visible && !minimized && !click_through {
        window.hide().map_err(|error| error.to_string())
    } else {
        show(app, None)
    }
}
