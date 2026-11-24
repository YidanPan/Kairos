use super::*;
use chrono::Utc;

fn time(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 22, hour, minute, 0)
        .single()
        .unwrap()
}

fn setup() -> (tempfile::TempDir, Connection) {
    let directory = tempfile::tempdir().unwrap();
    let db = storage::open(directory.path()).unwrap();
    db.execute(
        "INSERT INTO categories(id,name,color) VALUES ('c','Research','#000000')",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO tasks(id,title,category_id,due_date,due_time) VALUES ('t','Read','c','2026-11-22','10:00')",[]).unwrap();
    (directory, db)
}

fn task_rules(rules: Vec<Rule>, bypass: bool) -> TaskPreferences {
    TaskPreferences {
        overrides: true,
        allow_during_dnd: bypass,
        rules,
    }
}

#[test]
fn missed_reminders_are_grouped_once_and_future_items_stay_pending() {
    let (_directory, mut db) = setup();
    save_task(
        &db,
        "t",
        &task_rules(
            vec![
                Rule::Offset { minutes: 30 },
                Rule::Offset { minutes: 10 },
                Rule::Offset { minutes: 0 },
            ],
            false,
        ),
    )
    .unwrap();
    let grouped = collect_due(&mut db, time(9, 55)).unwrap();
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0].count, 2);
    assert_eq!(grouped[0].first_due_at, time(9, 30).timestamp());
    assert!(collect_due(&mut db, time(9, 59)).unwrap().is_empty());
    assert_eq!(collect_due(&mut db, time(10, 0)).unwrap()[0].count, 1);
    assert!(collect_due(&mut db, time(10, 30)).unwrap().is_empty());
}

#[test]
fn suppress_defers_delivery_and_silent_and_bypass_are_distinct() {
    let (dir, mut db) = setup();
    let saved = |db: &Connection, mode| {
        apply(
            db,
            dir.path(),
            Action::SavePreferences {
                preferences: Preferences {
                    manual_dnd: mode,
                    ..Preferences::default()
                },
            },
        )
        .unwrap()
    };
    saved(&db, Quiet::Suppress);
    save_task(
        &db,
        "t",
        &task_rules(
            vec![Rule::Offset { minutes: 30 }, Rule::Offset { minutes: 10 }],
            false,
        ),
    )
    .unwrap();
    assert!(collect_due(&mut db, time(9, 55)).unwrap().is_empty());
    saved(&db, Quiet::Silent);
    let notices = collect_due(&mut db, time(10, 0)).unwrap();
    assert_eq!(notices[0].count, 2);
    assert!(notices[0].silent);
    saved(&db, Quiet::Suppress);
    save_task(
        &db,
        "t",
        &task_rules(vec![Rule::Offset { minutes: 0 }], true),
    )
    .unwrap();
    let bypass = collect_due(&mut db, time(10, 1)).unwrap();
    assert!(!bypass[0].silent);
}

#[test]
fn category_updates_preserve_explicit_overrides_and_finished_tasks_are_excluded() {
    let (dir, mut db) = setup();
    let save = |db: &Connection, offsets| {
        apply(
            db,
            dir.path(),
            Action::SaveCategory {
                id: "c".into(),
                offsets,
            },
        )
        .unwrap()
    };
    save(&db, vec![30]);
    assert_eq!(collect_due(&mut db, time(9, 31)).unwrap().len(), 1);
    save_task(&db, "t", &task_rules(vec![], false)).unwrap();
    save(&db, vec![0, 10, 30]);
    assert!(collect_due(&mut db, time(10, 0)).unwrap().is_empty());
    db.execute(
        "UPDATE tasks SET completed_at='2026-11-22T10:00:00Z' WHERE id='t'",
        [],
    )
    .unwrap();
    assert!(inbox(&db).unwrap().is_empty());
    save_task(
        &db,
        "t",
        &task_rules(
            vec![Rule::Custom {
                at: "2026-11-22T10:10".into(),
            }],
            false,
        ),
    )
    .unwrap();
    assert!(collect_due(&mut db, time(10, 30)).unwrap().is_empty());
}

#[test]
fn a_snooze_is_persisted_once_and_resurfaces_as_a_new_reminder() {
    let (directory, mut db) = setup();
    save_task(
        &db,
        "t",
        &task_rules(vec![Rule::Offset { minutes: 0 }], false),
    )
    .unwrap();
    let notice = collect_due(&mut db, time(10, 1)).unwrap().remove(0);
    for minutes in [5, 15] {
        apply(
            &db,
            directory.path(),
            Action::Snooze {
                id: notice.id,
                minutes: Some(minutes),
                until: None,
            },
        )
        .unwrap();
    }
    assert!(inbox(&db).unwrap().is_empty());
    let at: i64 = db
        .query_row(
            "SELECT snoozed_for FROM reminder_batches WHERE id=?1",
            [notice.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(at <= Local::now().timestamp() + 300);
    let later = collect_due(&mut db, Utc.timestamp_opt(at, 0).single().unwrap()).unwrap();
    assert_eq!(later.len(), 1);
    assert_ne!(later[0].id, notice.id);
    assert!(
        collect_due(&mut db, Utc.timestamp_opt(at + 1, 0).single().unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn overnight_quiet_period_uses_the_day_it_started() {
    let preferences = Preferences {
        periods: vec![Period {
            days: vec![5],
            start: "22:00".into(),
            end: "07:00".into(),
            mode: Quiet::Suppress,
        }],
        ..Preferences::default()
    };
    assert!(quiet_at(&preferences, time(3, 0).naive_local()) == Quiet::Suppress);
    assert!(quiet_at(&preferences, time(7, 0).naive_local()) == Quiet::Off);
    assert!(quiet_at(&preferences, time(23, 0).naive_local()) == Quiet::Off);
}

#[test]
fn recurrence_inherits_relative_rules_and_dnd_exception_only() {
    let (_directory, mut db) = setup();
    db.execute("INSERT INTO tasks(id,title) VALUES ('next','Next')", [])
        .unwrap();
    save_task(
        &db,
        "t",
        &task_rules(
            vec![
                Rule::Offset { minutes: 0 },
                Rule::Custom {
                    at: "2026-11-22T10:10".into(),
                },
            ],
            true,
        ),
    )
    .unwrap();
    let tx = db.transaction().unwrap();
    inherit(&tx, "t", "next").unwrap();
    tx.commit().unwrap();
    let next = task_settings(&db, "next").unwrap();
    assert!(next.allow_during_dnd && next.overrides);
    assert!(matches!(
        next.rules.as_slice(),
        [Rule::Offset { minutes: 0 }]
    ));
}
