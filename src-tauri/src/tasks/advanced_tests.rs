use super::{recurrence::Frequency, *};

fn apply_on(database: &mut Connection, action: Action, day: &str) -> BoardResult<Board> {
    board(
        database,
        Some(action),
        recurrence::date(day)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap(),
    )
}

fn apply_today(database: &mut Connection, action: Action) -> Board {
    apply_on(database, action, "2026-11-21").unwrap()
}

fn create(database: &mut Connection, id: &str) {
    apply_today(
        database,
        Action::Create {
            id: id.into(),
            title: format!("Task {id}"),
        },
    );
}

fn add_subtask(database: &mut Connection, task: &str, id: &str, title: &str) {
    apply_today(
        database,
        Action::SaveSubtask {
            task_id: task.into(),
            id: id.into(),
            title: title.into(),
        },
    );
}

fn subtask_states(database: &Connection, task: &str) -> Vec<(String, bool)> {
    let mut query = database.prepare("SELECT title,completed_at IS NOT NULL FROM subtasks WHERE task_id=?1 ORDER BY position,id").unwrap();
    query
        .query_map([task], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn completed_count(database: &Connection) -> i64 {
    database
        .query_row(
            "SELECT count(*) FROM task_completions WHERE restored_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn start_series(database: &mut Connection) -> String {
    create(database, "first");
    apply_today(
        database,
        Action::Edit {
            id: "first".into(),
            change: Change::ScheduledDate(Some("2026-11-21".into())),
        },
    );
    apply_today(
        database,
        Action::Edit {
            id: "first".into(),
            change: Change::Deadline(Deadline {
                date: Some("2026-11-24".into()),
                time: Some("15:20".into()),
            }),
        },
    );
    let result = apply_today(
        database,
        Action::SaveRecurrence {
            id: "first".into(),
            scope: Scope::This,
            rule: Some(Rule {
                frequency: Frequency::Days,
                interval: 2,
                weekdays: vec![],
                month_day: None,
                end_date: None,
            }),
        },
    );
    result.tasks[0].series_id.clone().unwrap()
}

fn active_occurrence<'a>(board: &'a Board, series_id: &str) -> &'a Task {
    let active: Vec<_> = board
        .tasks
        .iter()
        .filter(|task| {
            task.series_id.as_deref() == Some(series_id)
                && task.completed_at.is_none()
                && task.trashed_at.is_none()
                && task.skipped_at.is_none()
        })
        .collect();
    assert_eq!(active.len(), 1);
    active[0]
}

fn complete(id: &str) -> Action {
    Action::SetCompleted {
        id: id.into(),
        completed: true,
    }
}

#[test]
fn confirming_task_edits_rolls_back_changes_when_the_repeat_rule_is_invalid() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    create(&mut database, "parent");
    let mut edit = serde_json::json!({
        "kind": "confirmTaskEdits",
        "id": "parent",
        "scope": "this",
        "changes": [{ "field": "title", "value": "Updated" }],
        "repeat": { "scope": "future", "rule": { "frequency": "days", "interval": 0, "weekdays": [], "monthDay": null, "endDate": null } },
        "reminders": { "overrides": true, "allowDuringDnd": true, "rules": [] },
        "subtasks": [{ "kind": "saveSubtask", "taskId": "parent", "id": "a", "title": "First step" }]
    });
    let result = apply_on(
        &mut database,
        serde_json::from_value(edit.clone()).unwrap(),
        "2026-11-21",
    );
    assert!(matches!(result, Err(message) if message.contains("Repeat interval")));
    let title: String = database
        .query_row("SELECT title FROM tasks WHERE id='parent'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(title, "Task parent");
    assert!(subtask_states(&database, "parent").is_empty());
    let reminders: i64 = database
        .query_row("SELECT count(*) FROM task_alert_preferences", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(reminders, 0);

    edit["repeat"]["rule"]["interval"] = 2.into();
    let result = apply_today(&mut database, serde_json::from_value(edit).unwrap());
    assert_eq!(result.tasks[0].title, "Updated");
    assert_eq!(result.tasks[0].recurrence.as_ref().unwrap().interval, 2);
    assert_eq!(
        subtask_states(&database, "parent"),
        vec![("First step".into(), false)]
    );
}

#[test]
fn confirming_subtasks_checks_completion_against_the_final_list() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    let series = start_series(&mut database);
    add_subtask(&mut database, "first", "a", "One");
    add_subtask(&mut database, "first", "b", "Two");
    apply_today(
        &mut database,
        Action::SetSubtaskCompleted {
            task_id: "first".into(),
            id: "a".into(),
            completed: true,
        },
    );
    let edit = serde_json::json!({
        "kind": "confirmTaskEdits", "id": "first", "scope": "this", "changes": [],
        "subtasks": [
            { "kind": "setSubtaskCompleted", "taskId": "first", "id": "b", "completed": true },
            { "kind": "setSubtaskCompleted", "taskId": "first", "id": "a", "completed": false }
        ]
    });
    let result = apply_today(&mut database, serde_json::from_value(edit).unwrap());
    assert_eq!(active_occurrence(&result, &series).id, "first");
    assert_eq!(completed_count(&database), 0);
    assert_eq!(
        subtask_states(&database, "first"),
        vec![("One".into(), false), ("Two".into(), true)]
    );
}

#[test]
fn final_subtask_completes_parent_and_restore_preserves_children() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    create(&mut database, "parent");
    add_subtask(&mut database, "parent", "a", "One");
    add_subtask(&mut database, "parent", "b", "Two");
    add_subtask(&mut database, "parent", "a", "Updated");
    apply_today(
        &mut database,
        Action::MoveSubtask {
            task_id: "parent".into(),
            id: "b".into(),
            neighbor_id: "a".into(),
        },
    );
    assert_eq!(
        subtask_states(&database, "parent"),
        vec![("Two".into(), false), ("Updated".into(), false)]
    );

    for (id, finished) in [("b", false), ("a", true)] {
        let result = apply_today(
            &mut database,
            Action::SetSubtaskCompleted {
                task_id: "parent".into(),
                id: id.into(),
                completed: true,
            },
        );
        assert_eq!(result.tasks[0].completed_at.is_some(), finished);
    }
    assert_eq!(completed_count(&database), 1);
    apply_today(
        &mut database,
        Action::SetCompleted {
            id: "parent".into(),
            completed: false,
        },
    );
    assert_eq!(completed_count(&database), 0);
    assert!(subtask_states(&database, "parent")
        .iter()
        .all(|(_, finished)| *finished));
}

#[test]
fn recurrence_advances_once_with_deadline_offset_and_unchecked_children() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    let series = start_series(&mut database);
    add_subtask(&mut database, "first", "a", "One");
    add_subtask(&mut database, "first", "b", "Two");
    let result = apply_on(&mut database, complete("first"), "2026-11-26").unwrap();
    let next = active_occurrence(&result, &series);
    assert_eq!(next.scheduled_date.as_deref(), Some("2026-11-27"));
    assert_eq!(next.due_date.as_deref(), Some("2026-11-30"));
    assert_eq!(next.due_time.as_deref(), Some("15:20"));
    assert_eq!(
        subtask_states(&database, &next.id),
        vec![("One".into(), false), ("Two".into(), false)]
    );
    assert!(subtask_states(&database, "first")
        .iter()
        .all(|(_, finished)| *finished));
    let id = next.id.clone();

    let repeated = apply_on(&mut database, complete("first"), "2026-11-26").unwrap();
    assert_eq!(active_occurrence(&repeated, &series).id, id);
    let skipped = apply_on(
        &mut database,
        Action::SkipOccurrence { id: id.clone() },
        "2026-11-27",
    )
    .unwrap();
    assert_eq!(
        active_occurrence(&skipped, &series)
            .occurrence_on
            .as_deref(),
        Some("2026-11-29")
    );
    let skipped_entry = skipped.tasks.iter().find(|task| task.id == id).unwrap();
    assert!(skipped_entry.skipped_at.is_some());
    assert!(skipped_entry.completed_at.is_none());
    assert_eq!(completed_count(&database), 1);
}

#[test]
fn series_edit_scopes_propagate_the_selected_field_and_preserve_completion() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    let series = start_series(&mut database);
    apply_today(
        &mut database,
        Action::EditOccurrence {
            id: "first".into(),
            change: Change::Title("One-off title".into()),
            scope: Scope::This,
        },
    );
    let board = apply_today(&mut database, complete("first"));
    let first = board.tasks.iter().find(|task| task.id == "first").unwrap();
    let finish = first.completed_at.clone();
    assert_eq!(active_occurrence(&board, &series).title, "Task first");
    let next = active_occurrence(&board, &series).id.clone();
    let updated = apply_today(
        &mut database,
        Action::EditOccurrence {
            id: next,
            change: Change::Title("New title".into()),
            scope: Scope::Future,
        },
    );
    assert_eq!(
        updated
            .tasks
            .iter()
            .find(|task| task.id == "first")
            .unwrap()
            .title,
        "One-off title"
    );
    let all = apply_today(
        &mut database,
        Action::EditOccurrence {
            id: "first".into(),
            change: Change::Priority(Priority::High),
            scope: Scope::Entire,
        },
    );
    assert!(all.tasks.iter().all(|task| task.priority == "high"));
    assert_eq!(
        all.tasks
            .iter()
            .find(|task| task.id == "first")
            .unwrap()
            .completed_at,
        finish
    );
    assert_eq!(active_occurrence(&all, &series).title, "New title");
}

#[test]
fn trash_scopes_keep_completed_history_and_restores_do_not_duplicate_series() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    let series = start_series(&mut database);
    let board = apply_today(&mut database, complete("first"));
    let next = active_occurrence(&board, &series).id.clone();
    let after_delete = apply_today(
        &mut database,
        Action::TrashTask {
            id: next.clone(),
            scope: Scope::This,
        },
    );
    let next_again = active_occurrence(&after_delete, &series).id.clone();
    assert_ne!(next, next_again);
    let restored = apply_today(
        &mut database,
        Action::RestoreTrash {
            ids: vec![next.clone()],
        },
    );
    let restored_task = restored.tasks.iter().find(|task| task.id == next).unwrap();
    assert!(restored_task.series_id.is_none());
    assert_eq!(
        restored_task.origin_series_id.as_deref(),
        Some(series.as_str())
    );
    assert_eq!(active_occurrence(&restored, &series).id, next_again);

    let stopped = apply_today(
        &mut database,
        Action::TrashTask {
            id: next_again.clone(),
            scope: Scope::Entire,
        },
    );
    let previous = stopped
        .tasks
        .iter()
        .find(|task| task.id == "first")
        .unwrap();
    assert!(previous.completed_at.is_some());
    assert!(previous.trashed_at.is_none());
    assert!(stopped
        .tasks
        .iter()
        .find(|task| task.id == next_again)
        .unwrap()
        .trashed_at
        .is_some());
    assert_eq!(completed_count(&database), 1);
}

#[test]
fn trash_batch_actions_only_affect_selected_states_and_preserve_completion_records() {
    let directory = tempfile::tempdir().unwrap();
    let mut database = storage::open(directory.path()).unwrap();
    create(&mut database, "done");
    create(&mut database, "active");
    apply_today(&mut database, complete("done"));
    apply_today(&mut database, Action::TrashCompleted { ids: None });
    let restored = apply_today(
        &mut database,
        Action::RestoreTrash {
            ids: vec!["done".into()],
        },
    );
    let done = restored
        .tasks
        .iter()
        .find(|task| task.id == "done")
        .unwrap();
    assert!(done.trashed_at.is_none());
    assert!(done.completed_at.is_some());
    apply_today(
        &mut database,
        Action::TrashCompleted {
            ids: Some(vec!["done".into(), "active".into()]),
        },
    );
    let remaining = apply_today(&mut database, Action::PurgeTrash { ids: None });
    assert_eq!(remaining.tasks.len(), 1);
    assert_eq!(remaining.tasks[0].id, "active");
    assert_eq!(completed_count(&database), 1);
}
