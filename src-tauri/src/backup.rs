use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, UNIX_EPOCH},
};

use chrono::{Local, Utc};
use rusqlite::{
    backup::{Backup, StepResult},
    params, Connection, OpenFlags, OptionalExtension, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tempfile::{NamedTempFile, TempDir};

use crate::{integration, reminders, storage};

mod restore;
#[cfg(test)]
mod tests;

type Result<T> = std::result::Result<T, String>;

const DIRECTORY_KEY: &str = "backup-directory";
const KEEP_BACKUPS: usize = 30;
static BACKUP_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    name: String,
    size: u64,
    modified_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    database: PathBuf,
    directory: PathBuf,
    custom_directory: bool,
    backups: Vec<BackupFile>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreMode {
    Replace,
    Merge,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    id: String,
    name: String,
    backup_tasks: u64,
    current_tasks: u64,
    merge_additions: u64,
}

struct Prepared {
    id: String,
    staging: TempDir,
}

#[derive(Default)]
pub struct BackupState(Mutex<Option<Prepared>>);

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn with_lock<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let _lock = BACKUP_LOCK.lock().map_err(error)?;
    operation()
}

fn directory_setting(db: &Connection) -> Result<Option<String>> {
    if storage::version(db).map_err(error)? < 2 {
        return Ok(None);
    }
    db.query_row(
        "SELECT value FROM app_settings WHERE key=?1",
        [DIRECTORY_KEY],
        |row| row.get(0),
    )
    .optional()
    .map_err(error)
}

fn backup_folder(data: &Path, setting: Option<&str>) -> PathBuf {
    setting
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("backups"))
}

fn backup_name() -> String {
    format!("Kairos-{}.sqlite3", Utc::now().format("%Y%m%dT%H%M%S%.9fZ"))
}

fn managed_name(name: &str) -> bool {
    name.strip_prefix("Kairos-")
        .and_then(|name| name.strip_suffix(".sqlite3"))
        .is_some_and(|stamp| {
            chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S%.9fZ").is_ok()
        })
}

fn list_backups(folder: &Path) -> Result<Vec<BackupFile>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(reason) => return Err(error(reason)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(error)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if managed_name(&name) && entry.file_type().map_err(error)?.is_file() {
            let meta = entry.metadata().map_err(error)?;
            let modified_at = meta
                .modified()
                .map_err(error)?
                .duration_since(UNIX_EPOCH)
                .map_err(error)?
                .as_secs();
            files.push(BackupFile {
                name,
                size: meta.len(),
                modified_at,
            });
        }
    }
    files.sort_by(|left, right| right.name.cmp(&left.name));
    Ok(files)
}

fn read_only(path: &Path) -> Result<Connection> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(error)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(error)?;
    db.pragma_update(None, "trusted_schema", false)
        .map_err(error)?;
    Ok(db)
}

fn copy_database(source: &Connection, target: &mut Connection) -> Result<()> {
    let backup = Backup::new(source, target).map_err(error)?;
    for _ in 0..30 {
        if matches!(backup.step(-1).map_err(error)?, StepResult::Done) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("Kairos could not copy the SQLite database while it was busy. Try again.".into())
}

fn check_integrity(db: &Connection) -> Result<()> {
    let result: String = db
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(error)?;
    if result != "ok"
        || db
            .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .map_err(error)?
            .is_some()
    {
        return Err("This backup contains inconsistent or damaged data.".into());
    }
    Ok(())
}

fn snapshot(source: &Connection, dest: &Path, overwrite: bool) -> Result<()> {
    let parent = dest.parent().ok_or("Choose a backup file location.")?;
    fs::create_dir_all(parent).map_err(error)?;
    let pending = NamedTempFile::new_in(parent).map_err(error)?;
    {
        let mut copy = Connection::open(pending.path()).map_err(error)?;
        copy_database(source, &mut copy)?;
        check_integrity(&copy)?;
    }
    pending.as_file().sync_all().map_err(error)?;
    if overwrite {
        pending.persist(dest)
    } else {
        pending.persist_noclobber(dest)
    }
    .map_err(error)?;
    Ok(())
}

fn create_in_folder(data: &Path, folder: &Path) -> Result<PathBuf> {
    let destination = folder.join(backup_name());
    snapshot(
        &read_only(&data.join(storage::FILE_NAME))?,
        &destination,
        false,
    )?;
    for old in list_backups(folder)?.into_iter().skip(KEEP_BACKUPS) {
        fs::remove_file(folder.join(old.name)).map_err(error)?;
    }
    Ok(destination)
}

pub fn before_change(data: &Path) -> Result<PathBuf> {
    with_lock(|| {
        let connection = read_only(&data.join(storage::FILE_NAME))?;
        let folder = backup_folder(data, directory_setting(&connection)?.as_deref());
        create_in_folder(data, &folder)
    })
    .map_err(|reason| format!("Could not back up Kairos before changing data. {reason}"))
}

fn status(data: &Path) -> Result<BackupStatus> {
    let db = storage::open(data).map_err(error)?;
    let setting = directory_setting(&db)?;
    let directory = backup_folder(data, setting.as_deref());
    Ok(BackupStatus {
        database: data.join(storage::FILE_NAME),
        backups: list_backups(&directory)?,
        directory,
        custom_directory: setting.is_some(),
    })
}

pub fn start(app: &AppHandle) {
    let Ok(data) = app.path().app_data_dir() else {
        return;
    };
    thread::spawn(move || loop {
        let attempt = with_lock(|| {
            let current = status(&data)?;
            let today = Local::now().date_naive();
            let today_backed_up = current
                .backups
                .first()
                .and_then(|entry| {
                    chrono::DateTime::from_timestamp(i64::try_from(entry.modified_at).ok()?, 0)
                })
                .is_some_and(|date| date.with_timezone(&Local).date_naive() == today);
            if !today_backed_up {
                create_in_folder(&data, &current.directory)?;
            }
            Ok(())
        });
        if let Err(reason) = attempt {
            eprintln!("Kairos automatic backup: {reason}");
        }
        thread::sleep(Duration::from_secs(1800));
    });
}

pub fn before_update(data: &Path) -> Result<()> {
    if data.join(storage::FILE_NAME).is_file() {
        before_change(data)?;
    }
    Ok(())
}

fn stage(data: &Path, source: &Path) -> Result<(Prepared, RestorePreview)> {
    let selected = read_only(source)?;
    let staging = tempfile::Builder::new()
        .prefix("kairos-restore-")
        .tempdir_in(data)
        .map_err(error)?;
    let path = staging.path().join(storage::FILE_NAME);
    let mut copy = Connection::open(&path).map_err(error)?;
    copy_database(&selected, &mut copy)?;
    copy.pragma_update(None, "trusted_schema", false)
        .map_err(error)?;
    storage::validate_backup_schema(&copy)?;
    check_integrity(&copy)?;
    storage::migrate_backup(&mut copy).map_err(error)?;
    let backup_tasks = copy
        .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
        .map_err(error)?;
    drop(copy);
    let current = storage::open(data).map_err(error)?;
    current
        .execute(
            "ATTACH DATABASE ?1 AS incoming",
            [path.to_string_lossy().as_ref()],
        )
        .map_err(error)?;
    let current_tasks = current
        .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
        .map_err(error)?;
    let merge_additions = restore::count_new_tasks(&current)?;
    let id = staging
        .path()
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok((
        Prepared {
            id: id.clone(),
            staging,
        },
        RestorePreview {
            id,
            name,
            backup_tasks,
            current_tasks,
            merge_additions,
        },
    ))
}

fn apply_restore(data: &Path, prepared: &Prepared, mode: RestoreMode) -> Result<BackupStatus> {
    with_lock(|| {
        let mut current = storage::open(data).map_err(error)?;
        let source = prepared.staging.path().join(storage::FILE_NAME);
        current
            .execute(
                "ATTACH DATABASE ?1 AS incoming",
                [source.to_string_lossy().as_ref()],
            )
            .map_err(error)?;
        let change = current
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        let setting = directory_setting(&change)?;
        let directory = backup_folder(data, setting.as_deref());
        create_in_folder(data, &directory)?;
        let result = BackupStatus {
            database: data.join(storage::FILE_NAME),
            backups: list_backups(&directory)?,
            directory,
            custom_directory: setting.is_some(),
        };
        match mode {
            RestoreMode::Replace => restore::replace(&change, setting.as_deref())?,
            RestoreMode::Merge => restore::merge(&change)?,
        }
        if change
            .query_row("PRAGMA main.foreign_key_check", [], |_| Ok(()))
            .optional()
            .map_err(error)?
            .is_some()
        {
            return Err("The restore would create inconsistent task relationships.".into());
        }
        change.commit().map_err(error)?;
        Ok(result)
    })
}

#[tauri::command]
pub async fn backup_status(app: AppHandle) -> Result<BackupStatus> {
    let data = app.path().app_data_dir().map_err(error)?;
    tauri::async_runtime::spawn_blocking(move || with_lock(|| status(&data)))
        .await
        .map_err(error)?
}

#[tauri::command]
pub async fn create_backup(app: AppHandle) -> Result<BackupStatus> {
    let data = app.path().app_data_dir().map_err(error)?;
    tauri::async_runtime::spawn_blocking(move || {
        before_change(&data)?;
        with_lock(|| status(&data))
    })
    .await
    .map_err(error)?
}

#[tauri::command]
pub async fn set_backup_directory(app: AppHandle, reset: bool) -> Result<BackupStatus> {
    let data = app.path().app_data_dir().map_err(error)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        use tauri_plugin_dialog::DialogExt;
        let folder = if reset { None } else {
            #[cfg(windows)]
            {
                let Some(folder) = app.dialog().file().set_title("Choose a Kairos backup folder").blocking_pick_folder() else { return with_lock(|| status(&data)); };
                Some(folder.into_path().map_err(error)?)
            }
            #[cfg(not(windows))]
            return Err("The backup folder picker is available on Windows.".into());
        };
        with_lock(|| {
            let db = storage::open(&data).map_err(error)?;
            if let Some(folder) = folder {
                let folder = folder.canonicalize().map_err(error)?;
                NamedTempFile::new_in(&folder).map_err(error)?;
                db.execute("INSERT INTO app_settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![DIRECTORY_KEY,folder.to_string_lossy()]).map_err(error)?;
            } else {
                db.execute("DELETE FROM app_settings WHERE key=?1", [DIRECTORY_KEY]).map_err(error)?;
            }
            status(&data)
        })
    }).await.map_err(error)?
}

#[tauri::command]
pub async fn export_backup(app: AppHandle) -> Result<bool> {
    #[cfg(windows)]
    {
        use tauri_plugin_dialog::DialogExt;
        let data = app.path().app_data_dir().map_err(error)?;
        tauri::async_runtime::spawn_blocking(move || {
            let name = backup_name();
            let Some(dest) = app
                .dialog()
                .file()
                .set_file_name(&name)
                .add_filter("Kairos backup", &["sqlite3"])
                .blocking_save_file()
            else {
                return Ok(false);
            };
            let dest = dest.into_path().map_err(error)?;
            let live = data.join(storage::FILE_NAME);
            let is_storage_sidecar = dest.parent().and_then(|parent| parent.canonicalize().ok())
                == data.canonicalize().ok()
                && dest
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        let name = name.to_ascii_lowercase();
                        ["", "-wal", "-shm", "-journal"]
                            .into_iter()
                            .any(|suffix| name == format!("{}{suffix}", storage::FILE_NAME))
                    });
            if is_storage_sidecar
                || dest
                    .canonicalize()
                    .ok()
                    .is_some_and(|path| live.canonicalize().ok().as_ref() == Some(&path))
            {
                return Err("Choose an export file outside the live Kairos database.".into());
            }
            with_lock(|| snapshot(&read_only(&live)?, &dest, true))?;
            Ok(true)
        })
        .await
        .map_err(error)?
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("The export dialog is available on Windows.".into())
    }
}

#[tauri::command]
pub async fn preview_restore(
    app: AppHandle,
    name: Option<String>,
) -> Result<Option<RestorePreview>> {
    let data = app.path().app_data_dir().map_err(error)?;
    tauri::async_runtime::spawn_blocking(move || {
        let source = if let Some(name) = name {
            if !managed_name(&name) {
                return Err("Choose a backup from the list.".into());
            }
            with_lock(|| Ok(status(&data)?.directory.join(name)))?
        } else {
            #[cfg(windows)]
            {
                use tauri_plugin_dialog::DialogExt;
                let Some(file) = app
                    .dialog()
                    .file()
                    .set_title("Open a Kairos backup")
                    .add_filter("Kairos backup", &["sqlite3", "db", "sqlite"])
                    .blocking_pick_file()
                else {
                    return Ok(None);
                };
                file.into_path().map_err(error)?
            }
            #[cfg(not(windows))]
            return Err("The import dialog is available on Windows.".into());
        };
        let (prepared, preview) = with_lock(|| stage(&data, &source))?;
        *app.state::<BackupState>().0.lock().map_err(error)? = Some(prepared);
        Ok(Some(preview))
    })
    .await
    .map_err(error)?
}

#[tauri::command]
pub async fn restore_backup(app: AppHandle, id: String, mode: RestoreMode) -> Result<BackupStatus> {
    let data = app.path().app_data_dir().map_err(error)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<BackupState>();
        let mut slot = state.0.lock().map_err(error)?;
        let selected = slot
            .as_ref()
            .filter(|entry| entry.id == id)
            .ok_or("Preview this backup again before restoring it.")?;
        let result = apply_restore(&data, selected, mode)?;
        *slot = None;
        if matches!(mode, RestoreMode::Replace) {
            reminders::invalidate_callbacks();
            integration::reload_preferences(&app);
        }
        Ok(result)
    })
    .await
    .map_err(error)?
}

#[tauri::command]
pub fn discard_restore(state: tauri::State<'_, BackupState>, id: String) -> Result<()> {
    let mut slot = state.0.lock().map_err(error)?;
    if slot.as_ref().is_some_and(|prepared| prepared.id == id) {
        *slot = None;
    }
    Ok(())
}
