use rusqlite::{Connection, TransactionBehavior};
use std::{error::Error, fmt, fs, path::Path, time::Duration};

const FILE_NAME: &str = "kairos.sqlite3";
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_initial.sql"),
    include_str!("../migrations/002_app_settings.sql"),
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
    migrate(&mut connection, MIGRATIONS)?;
    Ok(connection)
}

pub fn version(connection: &Connection) -> rusqlite::Result<u32> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn migrate(connection: &mut Connection, migrations: &[&str]) -> Result<(), StorageError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version = version(&transaction)?;
    if version as usize > migrations.len() {
        return Err(StorageError::NewerSchema(version));
    }

    for (index, migration) in migrations.iter().enumerate().skip(version as usize) {
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", index + 1)?;
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
        let result = migrate(&mut connection, &[MIGRATIONS[0], "invalid SQL"]);
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
        migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
        connection
            .execute(
                "INSERT INTO tasks(id, title) VALUES ('existing', 'Read')",
                [],
            )
            .unwrap();

        migrate(&mut connection, MIGRATIONS).unwrap();
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
        assert_eq!(version(&connection).unwrap(), 2);
    }

    #[test]
    fn leaves_newer_databases_intact() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 100).unwrap();
        assert!(matches!(
            migrate(&mut connection, MIGRATIONS),
            Err(StorageError::NewerSchema(100))
        ));
        assert_eq!(version(&connection).unwrap(), 100);
    }
}
