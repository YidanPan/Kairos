use super::*;

fn time(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}

fn create(id: &str) -> Action {
    Action::Create {
        id: id.into(),
        title: format!("Read {id}"),
    }
}

fn change(id: &str, change: Change) -> Action {
    Action::Edit {
        id: id.into(),
        change,
    }
}

fn category(id: &str, name: &str) -> Action {
    Action::SaveCategory {
        id: id.into(),
        name: name.into(),
        color: "#557788".into(),
    }
}

fn run(database: &mut Connection, action: Action) -> Board {
    board(database, Some(action), time("2026-11-21 10:00:00")).unwrap()
}

#[test]
fn quick_add_defaults_and_edits_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    let initial = run(&mut database, create("task"));
    assert_eq!(initial.tasks[0].priority, "medium");
    assert!(initial.tasks[0].scheduled_date.is_none());
    assert!(initial.tasks[0].due_date.is_none());
    assert!(initial.tasks[0].category_id.is_none());

    run(
        &mut database,
        change("task", Change::Title("读完课件".into())),
    );
    run(&mut database, change("task", Change::Notes("页 7".into())));
    run(
        &mut database,
        change("task", Change::Priority(Priority::High)),
    );
    run(
        &mut database,
        change("task", Change::ScheduledDate(Some("2026-11-22".into()))),
    );
    run(
        &mut database,
        change(
            "task",
            Change::Deadline(Deadline {
                date: Some("2026-11-24".into()),
                time: Some("15:20".into()),
            }),
        ),
    );
    drop(database);

    let mut database = storage::open(directory.path()).unwrap();
    let result = run(&mut database, create("task"));
    assert_eq!(result.tasks.len(), 1);
    let task = &result.tasks[0];
    assert_eq!(task.title, "读完课件");
    assert_eq!(task.notes, "页 7");
    assert_eq!(task.priority, "high");
    assert_eq!(task.scheduled_date.as_deref(), Some("2026-11-22"));
    assert_eq!(task.due_date.as_deref(), Some("2026-11-24"));
    assert_eq!(task.due_time.as_deref(), Some("15:20"));
}

#[test]
fn carry_forward_preserves_history_and_leaves_an_expired_schedule_intact() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    run(&mut database, create("carry"));
    run(&mut database, create("late"));
    database.execute("UPDATE tasks SET scheduled_date='2026-11-20',due_date='2026-11-21',due_time=CASE id WHEN 'late' THEN '09:30' ELSE '10:30' END", []).unwrap();

    for _ in 0..2 {
        let result = board(&mut database, None, time("2026-11-21 10:00:00")).unwrap();
        let late = result.tasks.iter().find(|task| task.id == "late").unwrap();
        let carry = result.tasks.iter().find(|task| task.id == "carry").unwrap();
        assert_eq!(late.scheduled_date.as_deref(), Some("2026-11-20"));
        assert_eq!(carry.scheduled_date.as_deref(), Some("2026-11-21"));
        assert_eq!(carry.rescheduled_on.as_deref(), Some("2026-11-21"));
        assert_eq!(carry.due_date.as_deref(), Some("2026-11-21"));
        assert_eq!(result.schedule_history.len(), 1);
        assert_eq!(result.schedule_history[0].previous_date, "2026-11-20");
        assert!(result.schedule_history[0].automatic);
    }
}

#[test]
fn category_removal_can_move_or_unassign_existing_tasks() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    run(&mut database, create("work"));
    run(&mut database, category("old", "Study"));
    run(&mut database, category("new", "Reading"));
    run(
        &mut database,
        change("work", Change::Category(Some("old".into()))),
    );

    let failed = Action::DeleteCategory {
        id: "old".into(),
        replacement: Some("missing".into()),
    };
    assert!(board(&mut database, Some(failed), time("2026-11-21 10:00:00")).is_err());
    let result = run(
        &mut database,
        Action::DeleteCategory {
            id: "old".into(),
            replacement: Some("new".into()),
        },
    );
    assert_eq!(result.tasks[0].category_id.as_deref(), Some("new"));
    assert_eq!(result.categories.len(), 1);
    let result = run(
        &mut database,
        Action::DeleteCategory {
            id: "new".into(),
            replacement: None,
        },
    );
    assert!(result.tasks[0].category_id.is_none());
    assert!(result.categories.is_empty());
}

#[test]
fn reordering_keeps_hidden_slots_and_new_tasks_at_the_top() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    for id in ["old", "hidden", "new"] {
        run(&mut database, create(id));
    }
    run(
        &mut database,
        Action::SetSort {
            mode: SortMode::Manual,
        },
    );
    run(
        &mut database,
        Action::Reorder {
            ids: vec!["old".into(), "new".into()],
        },
    );
    let result = run(&mut database, create("first"));
    assert!(matches!(result.sort_mode, SortMode::Manual));
    assert_eq!(
        result
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["first", "old", "hidden", "new"]
    );
}

#[test]
fn task_completion_is_idempotent_and_can_be_restored() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    run(&mut database, create("task"));
    let complete = || Action::SetCompleted {
        id: "task".into(),
        completed: true,
    };
    let first = run(&mut database, complete());
    let second = run(&mut database, complete());
    assert!(first.tasks[0].completed_at.is_some());
    assert_eq!(first.tasks[0].completed_at, second.tasks[0].completed_at);
    let restored = run(
        &mut database,
        Action::SetCompleted {
            id: "task".into(),
            completed: false,
        },
    );
    assert!(restored.tasks[0].completed_at.is_none());
}
