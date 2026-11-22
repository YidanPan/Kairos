#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod integration;
mod panel;
mod reminders;
mod storage;
mod tasks;

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    schema_version: u32,
    daily_greeting: Option<panel::DailyGreeting>,
}

#[tauri::command]
async fn storage_info(app: tauri::AppHandle) -> Result<StorageInfo, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || {
        let connection = storage::open(&data_dir).map_err(|error| error.to_string())?;
        let schema_version = storage::version(&connection).map_err(|error| error.to_string())?;
        let daily_greeting = panel::daily_greeting(&connection, chrono::Local::now().date_naive())
            .map_err(|error| error.to_string())?;
        Ok(StorageInfo {
            schema_version,
            daily_greeting,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn main() {
    let context = tauri::generate_context!();
    #[cfg(windows)]
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--backup-before-update"))
    {
        let result = std::env::var_os("APPDATA")
            .ok_or_else(|| "Windows application data is unavailable.".to_owned())
            .and_then(|directory| {
                backup::before_update(
                    &std::path::PathBuf::from(directory).join(&context.config().identifier),
                )
            });
        if let Err(error) = result {
            eprintln!("Kairos upgrade backup: {error}");
            std::process::exit(1);
        }
        return;
    }
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !reminders::activation::handle_launch(app, &args) {
                if let Err(error) = integration::show(app, None) {
                    eprintln!("Could not restore Kairos: {error}");
                }
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init());
    builder
        .setup(|app| {
            reminders::identify(app.handle());
            panel::setup(app)?;
            integration::setup(app.handle())?;
            app.manage(backup::BackupState::default());
            app.manage(reminders::activation::UiQueue::default());
            #[cfg(windows)]
            reminders::activation::handle_launch(
                app.handle(),
                &std::env::args().collect::<Vec<_>>(),
            );
            backup::start(app.handle());
            reminders::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    eprintln!("Could not hide Kairos: {error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            storage_info,
            backup::backup_status,
            backup::create_backup,
            backup::set_backup_directory,
            backup::export_backup,
            backup::preview_restore,
            backup::restore_backup,
            backup::discard_restore,
            integration::integration_status,
            integration::save_behavior,
            integration::set_click_through,
            integration::set_launch_at_login,
            panel::load_panel_preferences,
            panel::save_panel_preferences,
            panel::save_daily_greeting,
            panel::set_panel_backdrop,
            tasks::task_board,
            reminders::reminder_state,
            reminders::activation::pending_notification_actions,
        ])
        .run(context)
        .expect("Kairos could not start");
}
