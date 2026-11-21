#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod panel;
mod storage;

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfo {
    schema_version: u32,
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
        Ok(StorageInfo { schema_version })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn main() {
    tauri::Builder::default()
        .setup(panel::setup)
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
            panel::load_panel_preferences,
            panel::save_panel_preferences,
            panel::set_panel_backdrop,
        ])
        .run(tauri::generate_context!())
        .expect("Kairos could not start");
}
