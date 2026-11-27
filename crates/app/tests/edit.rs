use crate::common;

use app::{
    Session, demo,
    edit::Edit,
    meta::{TaskMeta, outline_path},
};
use common::{clock, demo_session, demo_session_seeded};
use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

fn assert_round_trip(edit: &Edit) {
    let (mut session, meta_before) = demo_session(300);
    let tasks_before = session.schedule().tasks().to_vec();
    let solution_before = session.solution().clone();

    let inverse = session.apply(edit.clone()).expect("the edit applies");
    match edit {
        Edit::Move { .. } => assert_ne!(session.schedule().tasks(), tasks_before),
        Edit::SetDuration { .. } | Edit::SetProgress { .. } => {
            assert_ne!(session.solution(), &solution_before);
        }
        Edit::Rename { task, .. } => {
            assert_ne!(
                session.meta()[task.0 as usize].name,
                meta_before[task.0 as usize].name
            );
        }
        // Structural edits need an outline; tests/structure.rs and tests/insert.rs round-trip them.
        // A link edit's inverse is another variant, so it is not a round trip of one value:
        // tests/links.rs applies each in turn. A baseline edit changes no date: tests/baseline.rs. A data-date edit has its own file: tests/data_date_edit.rs, a physical-percent edit tests/physical_edit.rs.
        Edit::Insert { .. }
        | Edit::Delete { .. }
        | Edit::Restore { .. }
        | Edit::Reparent { .. }
        | Edit::AddLink { .. }
        | Edit::RemoveLink { .. }
        | Edit::RestoreLink { .. }
        | Edit::SetLink { .. }
        | Edit::SetBaseline { .. }
        | Edit::ClearBaseline
        | Edit::SetDataDate { .. }
        | Edit::SetLimit { .. }
        | Edit::SetPhysical { .. } => unreachable!("not a one-value round trip"),
    }

    let redo = session.apply(inverse).expect("the inverse applies");
    assert_eq!(session.schedule().tasks(), tasks_before);
    assert_eq!(session.solution(), &solution_before);
    assert_eq!(session.meta(), meta_before);
    assert_eq!(redo, *edit);
}

#[test]
fn every_edit_returns_an_exact_inverse() {
    let (session, _) = demo_session(300);
    let task = TaskId(150);
    let start = session.solution().dates[task.0 as usize].es;
    let duration = session.schedule().tasks()[task.0 as usize].duration;

    assert_round_trip(&Edit::Move {
        task,
        snet: Some(start + 3 * DAY),
    });
    assert_round_trip(&Edit::SetDuration {
        task,
        minutes: duration + 137,
    });
    assert_round_trip(&Edit::Rename {
        task,
        name: "Exact inverse name".to_string(),
    });
    assert_round_trip(&Edit::SetProgress {
        task,
        actual_start: Some(start + 3 * DAY),
        actual_finish: None,
        remaining: Some(137),
    });
}

fn standard_calendar() -> Calendar {
    let day = vec![(8 * 60, 12 * 60), (13 * 60, 17 * 60)];
    Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid calendar")
}

fn add_task(schedule: &mut Schedule, calendar: engine::CalendarId, duration: i64) -> TaskId {
    schedule
        .add_task(Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("valid task")
}

fn link(schedule: &mut Schedule, pred: TaskId, succ: TaskId) {
    schedule
        .add_dependency(Dependency {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("valid dependency");
}

/// Two branches join at a milestone. Lengthening `edited` leaves the project
/// finish on the longer branch, but tightens the late dates and float of
/// `predecessor`; a forward cone cannot discover that backward-only change.
fn backward_change_session() -> (Session, TaskId, TaskId) {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let predecessor = add_task(&mut schedule, calendar, 480);
    let edited = add_task(&mut schedule, calendar, 480);
    let long_branch = add_task(&mut schedule, calendar, 4 * 480);
    let finish = add_task(&mut schedule, calendar, 0);
    link(&mut schedule, predecessor, edited);
    link(&mut schedule, edited, finish);
    link(&mut schedule, long_branch, finish);
    let solution = schedule.compute().expect("acyclic schedule");
    let order = timeline::order_by_start(&solution.dates);
    let meta = ["Predecessor", "Edited", "Long branch", "Finish"]
        .into_iter()
        .map(|name| TaskMeta {
            name: name.to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        })
        .collect();
    (
        Session::new(
            schedule,
            solution,
            Timeline::new(order, 800.0, 400.0, 1.0),
            meta,
            clock,
        ),
        predecessor,
        edited,
    )
}

#[test]
fn duration_edit_recomputes_backward_dates_and_matches_a_fresh_compute() {
    let (mut session, predecessor, edited) = backward_change_session();
    let predecessor_float = session.solution().get(predecessor).float;
    let project_finish = session.solution().project_finish;

    session
        .apply(Edit::SetDuration {
            task: edited,
            minutes: 2 * 480,
        })
        .expect("duration applies");

    let expected = session
        .schedule()
        .clone()
        .compute()
        .expect("fresh compute succeeds");
    assert_eq!(session.solution(), &expected);
    assert_eq!(session.solution().project_finish, project_finish);
    assert_ne!(session.solution().get(predecessor).float, predecessor_float);
}

#[test]
fn duration_edits_on_a_large_demo_always_match_a_fresh_compute() {
    let (mut session, _) = demo_session(2_000);
    let changes = [
        (TaskId(150), 0),
        (TaskId(750), session.schedule().tasks()[750].duration + 480),
        (
            TaskId(1_500),
            session.schedule().tasks()[1_500].duration + 73,
        ),
    ];

    for (task, minutes) in changes {
        session
            .apply(Edit::SetDuration { task, minutes })
            .expect("duration applies");
        let expected = session
            .schedule()
            .clone()
            .compute()
            .expect("fresh compute succeeds");
        assert_eq!(session.solution(), &expected, "task {task}");
        assert_eq!(
            session.schedule().tasks()[task.0 as usize].duration,
            minutes
        );
    }
}

fn assert_refused_without_change(session: &mut Session, edit: Edit) {
    let tasks = session.schedule().tasks().to_vec();
    let solution = session.solution().clone();
    let metadata = session.meta().to_vec();
    let revision = session.revision();
    let stamp = session.stamp();

    let history = session.history_len();

    // A person's edit: the rename rules for typed text apply to it, not to the raw `apply`.
    assert!(session.edit(edit).is_err());
    assert_eq!(session.history_len(), history);
    assert_eq!(session.schedule().tasks(), tasks);
    assert_eq!(session.solution(), &solution);
    assert_eq!(session.meta(), metadata);
    assert_eq!(session.revision(), revision);
    assert_eq!(session.stamp(), stamp);
}

#[test]
fn refused_edits_change_nothing() {
    let (mut session, _) = demo_session(100);
    let known = TaskId(50);
    let unknown = TaskId(100);
    let edits = [
        Edit::SetDuration {
            task: known,
            minutes: -1,
        },
        Edit::SetDuration {
            task: unknown,
            minutes: 60,
        },
        Edit::Move {
            task: unknown,
            snet: Some(0),
        },
        Edit::Rename {
            task: unknown,
            name: "Unknown".to_string(),
        },
        Edit::Rename {
            task: known,
            name: String::new(),
        },
        Edit::Rename {
            task: known,
            name: " \t ".to_string(),
        },
        Edit::Rename {
            task: known,
            name: "Bad / segment".to_string(),
        },
        Edit::Rename {
            task: known,
            name: "Bad\nname".to_string(),
        },
    ];

    for edit in edits {
        assert_refused_without_change(&mut session, edit);
    }
}

fn is_descendant(meta: &[TaskMeta], candidate: usize, ancestor: TaskId) -> bool {
    let mut parent = meta[candidate].parent;
    while let Some(id) = parent {
        if id == ancestor {
            return true;
        }
        parent = meta[id.0 as usize].parent;
    }
    false
}

#[test]
fn renaming_a_parent_changes_only_its_own_name_and_undo_restores_it() {
    let (mut session, before) = demo_session(300);
    let parent_index = before
        .iter()
        .position(|task| task.outline_level == 2)
        .expect("demo has a level-two parent");
    let parent = TaskId(parent_index as u32);
    let old_prefix = outline_path(&before, parent_index);
    let grandparent = before[parent_index]
        .parent
        .expect("a level-two task has a parent");
    let new_prefix = format!(
        "{} / Renamed phase",
        outline_path(&before, grandparent.0 as usize)
    );
    assert_ne!(new_prefix, old_prefix);

    let inverse = session
        .apply(Edit::Rename {
            task: parent,
            name: "Renamed phase".to_string(),
        })
        .expect("rename applies");

    let meta = session.meta();
    let mut descendants = 0;
    let mut outsiders = 0;
    for index in 0..meta.len() {
        if index == parent_index {
            assert_eq!(meta[index].name, "Renamed phase");
            assert_eq!(
                TaskMeta {
                    name: before[index].name.clone(),
                    ..meta[index].clone()
                },
                before[index],
                "a rename changes only the name"
            );
            assert_eq!(session.outline_path(index as u32), new_prefix);
        } else if is_descendant(&before, index, parent) {
            descendants += 1;
            assert_eq!(meta[index], before[index], "a descendant keeps its name");
            let old_path = outline_path(&before, index);
            let suffix = old_path
                .strip_prefix(&old_prefix)
                .expect("descendant path starts with its parent's path");
            assert_eq!(
                session.outline_path(index as u32),
                format!("{new_prefix}{suffix}"),
                "a descendant's path shows the new parent name"
            );
        } else {
            outsiders += 1;
            assert_eq!(meta[index], before[index]);
            assert_eq!(
                session.outline_path(index as u32),
                outline_path(&before, index)
            );
        }
    }
    assert!(descendants > 0);
    assert!(outsiders > 0);

    session.apply(inverse).expect("inverse applies");
    assert_eq!(session.meta(), before);
}

#[test]
fn rename_stores_padding_and_undo_restores_a_padded_imported_name_exactly() {
    let (_, original) = demo_session(300);
    let index = original
        .iter()
        .position(|task| task.outline_level == 3)
        .expect("demo has a leaf task");
    let task = TaskId(index as u32);

    // The padded name is in the meta the session is built with, as an imported file brings it.
    let (mut session, _) = demo_session_seeded(300, |meta| {
        meta[index].name = "  Padded leaf  ".to_string();
    });
    let before = session.meta().to_vec();
    assert_eq!(before[index].name.as_bytes(), b"  Padded leaf  ");
    let inverse = session
        .apply(Edit::Rename {
            task,
            name: "Plain".into(),
        })
        .expect("rename applies");
    assert_eq!(session.meta()[index].name, "Plain");

    session.apply(inverse).expect("inverse applies");
    assert_eq!(session.meta()[index].name.as_bytes(), b"  Padded leaf  ");
    assert_eq!(session.meta(), before);

    let (mut session, _) = demo_session_seeded(300, |meta| {
        meta[index].name = "Original leaf".to_string();
    });
    let before = session.meta().to_vec();
    let inverse = session
        .apply(Edit::Rename {
            task,
            name: "  Stored padded leaf  ".into(),
        })
        .expect("padded rename applies");
    assert_eq!(session.meta()[index].name, "  Stored padded leaf  ");

    session.apply(inverse).expect("inverse applies");
    assert_eq!(session.meta(), before);
}

#[test]
fn renaming_an_imported_name_that_holds_the_separator_undoes_byte_for_byte() {
    let (_, original) = demo_session(300);
    let index = original
        .iter()
        .position(|task| task.outline_level == 3)
        .expect("demo has a leaf task");
    let (mut session, _) = demo_session_seeded(300, |meta| {
        meta[index].name = "Pour / cure".to_string();
    });
    let before = session.meta().to_vec();
    let inverse = session
        .apply(Edit::Rename {
            task: TaskId(index as u32),
            name: "Pour".into(),
        })
        .expect("rename applies");
    assert_eq!(session.meta()[index].name, "Pour");

    session.apply(inverse).expect("inverse applies");
    assert_eq!(session.meta()[index].name, "Pour / cure");
    assert_eq!(session.meta(), before);

    // The same through the person's path: undo restores the name the file gave, redo the typed one.
    session
        .edit(Edit::Rename {
            task: TaskId(index as u32),
            name: "Pour".into(),
        })
        .expect("a plain rename applies");
    assert!(session.undo().expect("undo applies"));
    assert_eq!(session.meta()[index].name, "Pour / cure");
    assert_eq!(session.meta(), before);
    assert!(session.redo().expect("redo applies"));
    assert_eq!(session.meta()[index].name, "Pour");
}

#[test]
fn applied_edits_bump_both_counters_once_and_refusals_do_not() {
    let (mut session, _) = demo_session(100);
    let task = TaskId(50);
    let edits = [
        Edit::Move {
            task,
            snet: Some(session.solution().get(task).es + DAY),
        },
        Edit::SetDuration {
            task,
            minutes: session.schedule().tasks()[task.0 as usize].duration + 1,
        },
        Edit::Rename {
            task,
            name: "Counter rename".to_string(),
        },
    ];

    for edit in edits {
        let (revision, stamp) = (session.revision(), session.stamp());
        session.apply(edit).expect("edit applies");
        assert_eq!(session.revision(), revision.wrapping_add(1));
        assert_eq!(session.stamp(), stamp.wrapping_add(1));
    }

    let (revision, stamp) = (session.revision(), session.stamp());
    assert!(
        session
            .apply(Edit::SetDuration { task, minutes: -1 },)
            .is_err()
    );
    assert_eq!((session.revision(), session.stamp()), (revision, stamp));
}

const MINUTES_PER_PIXEL: f64 = 100.0;
const TRAVEL_PIXELS: f64 = 200.0;

fn drag_setup() -> (Session, TaskId, i64, (f64, f64)) {
    let (schedule, solution, order, meta) = demo(2_000);
    let mut timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    let row = (100..timeline.order().len())
        .find(|&row| {
            let task = timeline.order()[row] as usize;
            solution.dates[task].ef - solution.dates[task].es >= 800
        })
        .expect("demo has a wide mid-schedule bar");
    let task = TaskId(timeline.order()[row]);
    let start = solution.get(task).es;
    let viewport = &mut timeline.viewport;
    viewport.px_per_minute = 1.0 / MINUTES_PER_PIXEL;
    viewport.t0 = start as f64 - 500.0 * MINUTES_PER_PIXEL;
    viewport.scroll_to_row(row);
    let point = (
        viewport.x_of(start as f64) + 2.0,
        viewport.row_top(row) + viewport.row_height / 2.0,
    );
    let session = Session::new(schedule, solution, timeline, meta, clock);
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task.0)
    );
    (session, task, start, point)
}

#[test]
fn move_through_apply_matches_the_drag_path() {
    let (mut dragged, task, start, (x, y)) = drag_setup();
    dragged.pointer_down(x, y, false);
    dragged.pointer_move(x + TRAVEL_PIXELS, y);
    dragged.pointer_up();

    let (mut edited, same_task, same_start, _) = drag_setup();
    assert_eq!((same_task, same_start), (task, start));
    edited
        .apply(Edit::Move {
            task,
            // The drag asks for the day start of the minute the pointer is at.
            snet: Some(
                edited
                    .schedule()
                    .calendar_of(task)
                    .day_start_at(start + (TRAVEL_PIXELS * MINUTES_PER_PIXEL) as i64),
            ),
        })
        .expect("move applies");

    assert_eq!(dragged.schedule().tasks(), edited.schedule().tasks());
    assert_eq!(dragged.solution(), edited.solution());
}

#[test]
fn rename_refuses_a_leaf_that_joins_the_separator_and_keeps_history() {
    let (mut session, meta) = demo_session(300);
    let nested = meta
        .iter()
        .position(|task| task.parent.is_some())
        .expect("demo has a nested task");
    let top = meta
        .iter()
        .position(|task| task.parent.is_none())
        .expect("demo has a top-level task");

    for index in [nested, top] {
        let task = TaskId(index as u32);
        session
            .edit(Edit::Rename {
                task,
                name: "Before".to_string(),
            })
            .expect("a plain rename applies");
        let history = session.history_len();

        for bad in ["/ X", "X /"] {
            let before = session.meta().to_vec();
            assert!(
                session
                    .edit(Edit::Rename {
                        task,
                        name: bad.to_string(),
                    },)
                    .is_err(),
                "{bad:?} is refused for task {index}"
            );
            assert_eq!(session.meta(), before, "{bad:?} changes no name");
            assert_eq!(session.history_len(), history, "{bad:?} leaves history");
        }

        for good in ["A/B", "a/ b", "a /b"] {
            session
                .edit(Edit::Rename {
                    task,
                    name: good.to_string(),
                })
                .unwrap_or_else(|error| panic!("{good:?} applies: {error}"));
            assert_eq!(session.meta()[index].name, good, "{good:?} reads back");
        }
    }
}
