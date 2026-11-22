use rusqlite::{Connection, TransactionBehavior};
use std::{error::Error, fmt, fs, path::Path, time::Duration};

pub const FILE_NAME: &str = "kairos.sqlite3";
pub const NOTIFICATION_SOUND: &str = "notification.wav";
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_initial.sql"),
    include_str!("../migrations/002_app_settings.sql"),
    include_str!("../migrations/003_tasks.sql"),
    include_str!("../migrations/004_advanced_tasks.sql"),
    include_str!("../migrations/005_reminders.sql"),
    include_str!("../migrations/006_statistics.sql"),
    include_str!("../migrations/007_data.sql"),
    include_str!("../migrations/008_notification_activation.sql"),
];

#[derive(Debug)]
pub enum StorageError {
    Directory(std::io::Error),
    Database(rusqlite::Error),
    NewerSchema(u32),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory(error) => {
                write!(formatter, "Could not create the data directory ({error})")
            }
            Self::Database(error) => write!(
                formatter,
                "SQLite could not open or migrate the database ({error})"
            ),
            Self::NewerSchema(version) => write!(
                formatter,
                "Database version {version} requires a newer version of Kairos"
            ),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Directory(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::NewerSchema(_) => None,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

pub fn open(data_dir: &Path) -> Result<Connection, StorageError> {
    fs::create_dir_all(data_dir).map_err(StorageError::Directory)?;
    let mut connection = Connection::open(data_dir.join(FILE_NAME))?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    let legacy_sound = data_dir.join(NOTIFICATION_SOUND);
    let legacy_sound = (version(&connection)? < 7)
        .then(|| {
            fs::metadata(&legacy_sound)
                .ok()
                .filter(|meta| meta.len() <= 12_000_000)
        })
        .flatten()
        .and_then(|_| fs::read(legacy_sound).ok());
    migrate(&mut connection, MIGRATIONS, legacy_sound.as_deref())?;
    Ok(connection)
}

pub fn version(connection: &Connection) -> rusqlite::Result<u32> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

pub fn validate_backup_schema(connection: &Connection) -> Result<(), String> {
    let version = version(connection).map_err(|error| error.to_string())? as usize;
    if version == 0 || version > MIGRATIONS.len() {
        return Err("This backup requires a supported Kairos database version.".into());
    }
    let mut expected = Connection::open_in_memory().map_err(|error| error.to_string())?;
    migrate(&mut expected, &MIGRATIONS[..version], None).map_err(|error| error.to_string())?;
    let definition = |connection: &Connection| -> rusqlite::Result<Vec<String>> {
        let mut query = connection.prepare("SELECT sql FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' AND sql IS NOT NULL ORDER BY type,name")?;
        let rows = query
            .query_map([], |row| Ok(row.get::<_, String>(0)?.replace("\r\n", "\n")))?
            .collect();
        rows
    };
    if definition(connection).map_err(|error| error.to_string())?
        != definition(&expected).map_err(|error| error.to_string())?
    {
        return Err("This file does not contain an intact Kairos database schema.".into());
    }
    Ok(())
}

pub fn migrate_backup(connection: &mut Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    migrate(connection, MIGRATIONS, None)
}

fn migrate(
    connection: &mut Connection,
    migrations: &[&str],
    legacy_sound: Option<&[u8]>,
) -> Result<(), StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version = version(&transaction)?;
    if version as usize > migrations.len() {
        return Err(StorageError::NewerSchema(version));
    }

    for (index, migration) in migrations.iter().enumerate().skip(version as usize) {
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", index + 1)?;
    }
    if version < 7 && migrations.len() >= 7 {
        if let Some(sound) = legacy_sound
            .filter(|bytes| bytes.len() >= 44 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE")
        {
            transaction.execute(
                "INSERT INTO app_assets(name,contents) VALUES (?1,?2)",
                rusqlite::params![NOTIFICATION_SOUND, sound],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reopens_the_database_without_losing_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Kairos");
        let connection = open(&path).unwrap();
        assert_eq!(version(&connection).unwrap() as usize, MIGRATIONS.len());
        connection
            .execute(
                "INSERT INTO tasks (id, title) VALUES (?1, ?2)",
                ["task-1", "读书"],
            )
            .unwrap();
        drop(connection);

        let connection = open(&path).unwrap();
        let title: String = connection
            .query_row("SELECT title FROM tasks WHERE id = 'task-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(title, "读书");
    }

    #[test]
    fn rolls_back_the_schema_and_version_together_on_failure() {
        let mut connection = Connection::open_in_memory().unwrap();
        let result = migrate(&mut connection, &[MIGRATIONS[0], "invalid SQL"], None);
        assert!(matches!(result, Err(StorageError::Database(_))));
        assert_eq!(version(&connection).unwrap(), 0);
        let table_count: i32 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'tasks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 0);
    }

    #[test]
    fn adds_panel_settings_without_changing_existing_tasks() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection, &MIGRATIONS[..1], None).unwrap();
        connection
            .execute(
                "INSERT INTO tasks(id, title) VALUES ('existing', 'Read')",
                [],
            )
            .unwrap();

        migrate(&mut connection, MIGRATIONS, None).unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key, value) VALUES ('panel', '{}')",
                [],
            )
            .unwrap();
        let title: String = connection
            .query_row("SELECT title FROM tasks WHERE id = 'existing'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(title, "Read");
        assert_eq!(version(&connection).unwrap() as usize, MIGRATIONS.len());
    }

    #[test]
    fn leaves_newer_databases_intact() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 100).unwrap();
        assert!(matches!(
            migrate(&mut connection, MIGRATIONS, None),
            Err(StorageError::NewerSchema(100))
        ));
        assert_eq!(version(&connection).unwrap(), 100);
    }

    #[test]
    fn migrates_existing_completion_timestamps_into_history() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        migrate(&mut connection, &MIGRATIONS[..3], None).unwrap();
        let stamp = "2026-11-21T12:00:00Z";
        connection.execute("INSERT INTO tasks(id,title,scheduled_date,completed_at) VALUES ('old','Read','2026-11-21',?1)", [stamp]).unwrap();
        migrate(&mut connection, MIGRATIONS, None).unwrap();
        let (finished, planned, key): (String, String, String) = connection
            .query_row(
                "SELECT completed_at,planned_on,task_key FROM task_completions WHERE task_id='old'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(finished, stamp);
        assert_eq!(planned, "2026-11-21");
        assert_eq!(key, "old");
    }
}
