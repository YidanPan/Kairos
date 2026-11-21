#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
        .invoke_handler(tauri::generate_handler![storage_info])
        .run(tauri::generate_context!())
        .expect("Kairos could not start");
}
