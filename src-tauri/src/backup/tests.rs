use super::*;

fn fixture(directory: &Path) -> Connection {
    storage::open(directory).unwrap()
}

fn add_task(database: &Connection, id: &str, title: &str) {
    database
        .execute(
            "INSERT INTO tasks(id,title) VALUES (?1,?2)",
            params![id, title],
        )
        .unwrap();
}

fn titles(database: &Connection) -> Vec<String> {
    let mut statement = database
        .prepare("SELECT title FROM tasks ORDER BY id")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn merge_keeps_current_records_and_restores_reminder_relationships_once() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let incoming = fixture(source.path());
    let current = fixture(target.path());
    incoming
        .execute(
            "INSERT INTO categories VALUES ('source','Work','#555555')",
            [],
        )
        .unwrap();
    current
        .execute(
            "INSERT INTO categories VALUES ('local','work','#223344')",
            [],
        )
        .unwrap();
    add_task(&incoming, "same", "Old title");
    add_task(&current, "same", "Current title");
    add_task(&incoming, "added", "From backup");
    for (db, id, title, occurrence) in [
        (&incoming, "stale", "Stale occurrence", "2026-11-21"),
        (&current, "active", "Current occurrence", "2026-11-22"),
    ] {
        let rule =
            r#"{"frequency":"days","interval":1,"weekdays":[],"monthDay":null,"endDate":null}"#;
        db.execute("INSERT INTO recurrence_series(id,anchor_date,rule,title,priority,notes) VALUES ('same-series','2026-11-21',?1,'Routine','medium','')", [rule]).unwrap();
        add_task(db, id, title);
        db.execute(
            "UPDATE tasks SET series_id='same-series',occurrence_on=?2 WHERE id=?1",
            params![id, occurrence],
        )
        .unwrap();
    }
    incoming
        .execute("UPDATE tasks SET category_id='source' WHERE id='added'", [])
        .unwrap();
    incoming.execute("INSERT INTO subtasks(id,task_id,title,position) VALUES ('sub','added','From backup child',1)", []).unwrap();
    for (db, id) in [(&incoming, "added"), (&current, "same")] {
        db.execute("INSERT INTO reminder_batches(id,task_id,created_at,first_due_at,count,silent,snoozed_for) VALUES (1,?1,100,100,1,1,400)", [id]).unwrap();
    }
    incoming
        .execute(
            "INSERT INTO reminder_receipts VALUES ('added','snooze:1',400)",
            [],
        )
        .unwrap();
    let source_file = before_change(source.path()).unwrap();
    let (prepared, preview) = stage(target.path(), &source_file).unwrap();
    assert_eq!(
        (
            preview.current_tasks,
            preview.backup_tasks,
            preview.merge_additions
        ),
        (2, 3, 1)
    );
    apply_restore(target.path(), &prepared, RestoreMode::Merge).unwrap();
    apply_restore(target.path(), &prepared, RestoreMode::Merge).unwrap();
    assert_eq!(
        titles(&current),
        ["Current occurrence", "From backup", "Current title"]
    );
    let mapped: String = current
        .query_row(
            "SELECT category_id FROM tasks WHERE id='added'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mapped, "local");
    let receipt: String = current
        .query_row(
            "SELECT token FROM reminder_receipts WHERE task_id='added'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(receipt, "snooze:2");
    assert_eq!(
        current
            .query_row("SELECT count(*) FROM reminder_batches", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        current
            .query_row("SELECT count(*) FROM subtasks", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    check_integrity(&current).unwrap();
}

#[test]
fn replace_recovers_settings_and_wav_and_failed_restore_is_atomic() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let incoming = fixture(source.path());
    let current = fixture(target.path());
    add_task(&incoming, "a", "Historical task");
    add_task(&current, "b", "Current task");
    incoming
        .execute("INSERT INTO app_settings VALUES ('task-sort','manual')", [])
        .unwrap();
    incoming
        .execute(
            "INSERT INTO app_assets VALUES ('notification.wav',X'52494646')",
            [],
        )
        .unwrap();
    let source_file = before_change(source.path()).unwrap();
    let (prepared, _) = stage(target.path(), &source_file).unwrap();
    let damaged = fixture(prepared.staging.path());
    damaged.execute("DROP TABLE app_assets", []).unwrap();
    assert!(apply_restore(target.path(), &prepared, RestoreMode::Replace).is_err());
    assert_eq!(titles(&current), ["Current task"]);
    let (prepared, _) = stage(target.path(), &source_file).unwrap();
    let safety = apply_restore(target.path(), &prepared, RestoreMode::Replace).unwrap();
    assert_eq!(titles(&current), ["Historical task"]);
    assert_eq!(
        current
            .query_row(
                "SELECT value FROM app_settings WHERE key='task-sort'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "manual"
    );
    assert_eq!(
        current
            .query_row("SELECT length(contents) FROM app_assets", [], |row| row
                .get::<_, usize>(
                0
            ))
            .unwrap(),
        4
    );
    let before = read_only(&safety.directory.join(&safety.backups[0].name)).unwrap();
    assert_eq!(titles(&before), ["Current task"]);
}

#[test]
fn malformed_files_and_schema_are_rejected_and_rotation_keeps_unrelated_files() {
    let folder = tempfile::tempdir().unwrap();
    let current = fixture(folder.path());
    add_task(&current, "a", "Kept");
    let corrupt = folder.path().join("not-a-database.sqlite3");
    fs::write(&corrupt, b"invalid file").unwrap();
    assert!(stage(folder.path(), &corrupt).is_err());
    let fake = folder.path().join("other.sqlite3");
    let fake_db = Connection::open(&fake).unwrap();
    fake_db
        .execute_batch("CREATE TABLE tasks(id); PRAGMA user_version=7;")
        .unwrap();
    assert!(stage(folder.path(), &fake).is_err());
    assert_eq!(titles(&current), ["Kept"]);
    let backup_folder = folder.path().join("backups");
    fs::create_dir(&backup_folder).unwrap();
    for day in 1..=31 {
        fs::write(
            backup_folder.join(format!("Kairos-202510{day:02}T101010.000000000Z.sqlite3")),
            [],
        )
        .unwrap();
    }
    let unrelated = backup_folder.join("my-archive.sqlite3");
    fs::write(&unrelated, b"keep").unwrap();
    before_change(folder.path()).unwrap();
    assert_eq!(list_backups(&backup_folder).unwrap().len(), 30);
    assert!(unrelated.is_file());
}
