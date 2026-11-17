//! Delete, restore and undo on `Session` (gs#302, gs#50 PR 2).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    Place, Session,
    edit::{Edit, EditError},
    outline::Outline,
};
use common::{demo_session, outline_session};
use engine::{DepId, ScheduleError, TaskDates, TaskId};

const N: usize = 300;

/// Everything a delete changes, in comparable form.
#[derive(Debug, PartialEq)]
struct Snapshot {
    tasks: String,
    dependencies: String,
    meta: Vec<app::meta::TaskMeta>,
    preorder: Vec<TaskId>,
    order: Vec<u32>,
    dates: Vec<TaskDates>,
    spans: Vec<Option<(i64, i64)>>,
    selection: Option<(u32, u32)>,
}

fn snapshot(session: &Session) -> Snapshot {
    let slots = session.schedule().tasks().len() as u32;
    Snapshot {
        tasks: format!("{:?}", session.schedule().tasks()),
        dependencies: format!("{:?}", session.schedule().dependencies()),
        meta: session.meta().to_vec(),
        preorder: session.outline().unwrap().preorder().to_vec(),
        order: session.timeline().order().to_vec(),
        dates: session.solution().dates.clone(),
        spans: (0..slots)
            .map(|t| session.timeline().summaries().get(t))
            .collect(),
        selection: session.selection().map(|s| (s.task, s.row)),
    }
}

/// The session's derived state against a model rebuilt from the schedule alone.
fn assert_model(session: &Session) {
    let outline = session.outline().unwrap();
    let live = session.schedule().live_task_count();
    assert_eq!(outline.preorder().len(), live);
    assert_eq!(session.task_count(), live);
    let rebuilt = Outline::new(session.meta(), outline.preorder(), session.schedule())
        .expect("the outline is valid");
    assert_eq!(rebuilt.preorder(), outline.preorder());
    let timeline = session.timeline();
    for (row, &task) in timeline.order().iter().enumerate() {
        assert_eq!(timeline.row_of_task(task), Some(row));
        assert!(!session.schedule().is_deleted(TaskId(task)), "dead row");
    }
    let mut fresh = session.schedule().clone();
    fresh.clear_cached_order();
    assert_eq!(&fresh.compute().unwrap(), session.solution());
    let mut spans = timeline::Summaries::new(session.schedule().tasks().len());
    rebuilt.rollup_full(&session.solution().dates, &mut spans);
    for t in 0..session.schedule().tasks().len() as u32 {
        assert_eq!(timeline.summaries().get(t), spans.get(t), "span {t}");
    }
    if let Some(selection) = session.selection() {
        assert!(!session.schedule().is_deleted(TaskId(selection.task)));
        assert_eq!(
            timeline.row_of_task(selection.task),
            Some(selection.row as usize)
        );
    }
}

fn leaves(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().unwrap();
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&t| !outline.is_summary(t))
        .collect()
}

fn first_summary(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| outline.is_summary(t))
        .expect("a summary")
}

/// A leaf with a live predecessor and successor.
fn linked_leaf(session: &Session) -> TaskId {
    let deps = session.schedule().dependencies();
    leaves(session)
        .into_iter()
        .find(|t| deps.iter().any(|d| d.succ == *t) && deps.iter().any(|d| d.pred == *t))
        .expect("a linked leaf")
}

#[test]
fn deleting_a_leaf_solves_like_a_full_compute() {
    let (mut session, _) = outline_session(N);
    let mut late_changed = false;
    for task in leaves(&session).into_iter().take(40) {
        if !session
            .schedule()
            .dependencies()
            .iter()
            .any(|d| d.succ == task || d.pred == task)
        {
            continue;
        }
        let preds: Vec<_> = session
            .schedule()
            .dependencies()
            .iter()
            .filter(|d| d.succ == task)
            .map(|d| d.pred)
            .collect();
        let before = session.solution().clone();
        let inverse = session
            .apply(Edit::Delete { task })
            .expect("a leaf deletes");
        assert_model(&session);
        let dead = session.solution().dates[task.0 as usize];
        assert_eq!(
            (dead.es, dead.ef, dead.ls, dead.lf, dead.float),
            (0, 0, 0, 0, 0)
        );
        late_changed |= preds.iter().any(|p| {
            let (a, b) = (
                &before.dates[p.0 as usize],
                &session.solution().dates[p.0 as usize],
            );
            (a.ls, a.lf, a.float) != (b.ls, b.lf, b.float)
        });
        session.apply(inverse).expect("the inverse restores");
    }
    assert!(late_changed, "some delete moved a predecessor's late dates");
}

#[test]
fn delete_then_undo_restores_exactly() {
    let (mut session, _) = outline_session(N);
    let task = linked_leaf(&session);
    session.select(task.0);
    let before = snapshot(&session);
    assert_eq!(session.delete_selected(), Ok(task));
    let deleted = snapshot(&session);
    assert_ne!(deleted, before);
    assert!(session.schedule().is_deleted(task));
    assert!(
        session
            .outline()
            .unwrap()
            .preorder()
            .iter()
            .all(|&t| t != task)
    );
    assert!(session.timeline().row_of_task(task.0).is_none());
    assert_model(&session);

    assert_eq!(session.undo(), Ok(true));
    assert_eq!(snapshot(&session), before);
    assert_model(&session);
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(snapshot(&session), deleted);
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(snapshot(&session), before);
}

#[test]
fn deleting_a_driving_leaf_moves_a_successors_dates() {
    let (mut session, _) = outline_session(N);
    let moved = leaves(&session).into_iter().any(|task| {
        let successors: Vec<TaskId> = session
            .schedule()
            .dependencies()
            .iter()
            .filter(|d| d.pred == task)
            .map(|d| d.succ)
            .collect();
        let early = |s: &Session, t: TaskId| {
            let d = s.solution().dates[t.0 as usize];
            (d.es, d.ef)
        };
        let before: Vec<_> = successors.iter().map(|&t| early(&session, t)).collect();
        session.select(task.0);
        session.delete_selected().expect("a leaf deletes");
        let after: Vec<_> = successors.iter().map(|&t| early(&session, t)).collect();
        assert_model(&session);
        assert!(session.undo().unwrap());
        before != after
    });
    assert!(moved, "some delete moved a successor's start or finish");
}

#[test]
fn delete_moves_the_selection_to_the_next_row_else_the_previous_else_none() {
    let (mut session, _) = outline_session(N);
    let order = session.timeline().order().to_vec();
    let row = order
        .iter()
        .position(|&t| {
            t != first_summary(&session).0 && !session.outline().unwrap().is_summary(TaskId(t))
        })
        .unwrap();
    let task = order[row];
    let next = order.get(row + 1).copied().unwrap();
    session.select(task);
    session.delete_selected().unwrap();
    assert_eq!(session.selection().map(|s| s.task), Some(next));

    // The last row falls back to the previous one.
    let last = *session.timeline().order().last().unwrap();
    let previous = session.timeline().order()[session.timeline().order().len() - 2];
    session.select(last);
    session.delete_selected().unwrap();
    assert_eq!(session.selection().map(|s| s.task), Some(previous));

    // Delete everything that can be deleted: the last delete leaves no selection.
    while let Some(leaf) = session
        .timeline()
        .order()
        .iter()
        .copied()
        .find(|&t| !session.outline().unwrap().is_summary(TaskId(t)))
    {
        session.select(leaf);
        session.delete_selected().unwrap();
        assert_model(&session);
    }
    // Only childless summaries are left, now leaves; they delete too, down to none.
    while let Some(&top) = session.timeline().order().last() {
        session.select(top);
        session.delete_selected().unwrap();
    }
    assert!(session.selection().is_none());
    assert_eq!(session.task_count(), 0);
    assert_eq!(session.delete_selected(), Err(EditError::NoSelection));
}

#[test]
fn a_summary_is_refused_with_its_message_and_nothing_changes() {
    let (mut session, _) = outline_session(N);
    let summary = first_summary(&session);
    let count = session.outline().unwrap().children(summary).len();
    let name = session.meta()[summary.0 as usize].name.clone();
    session.select(summary.0);
    let before = snapshot(&session);
    let error = session.delete_selected().unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "\"{name}\" is a summary with {count} sub-tasks. Delete or move its sub-tasks first."
        )
    );
    assert_eq!(
        error,
        EditError::HasChildren {
            task: summary,
            name,
            count
        }
    );
    assert_eq!(snapshot(&session), before);
    assert_eq!(session.history_len(), (0, 0));
}

#[test]
fn the_last_child_goes_and_the_parent_reads_as_a_leaf_until_undo() {
    let (mut session, _) = outline_session(N);
    let outline = session.outline().unwrap();
    let parent = *outline
        .preorder()
        .iter()
        .find(|&&t| {
            outline.is_summary(t) && outline.children(t).iter().all(|&c| !outline.is_summary(c))
        })
        .expect("a summary of leaves");
    let children = outline.children(parent).to_vec();
    let before = snapshot(&session);
    for &child in &children {
        session.select(child.0);
        session.delete_selected().unwrap();
        assert_model(&session);
    }
    assert!(!session.outline().unwrap().is_summary(parent));
    assert_eq!(session.timeline().summaries().get(parent.0), None);
    for _ in &children {
        assert_eq!(session.undo(), Ok(true));
    }
    assert!(session.outline().unwrap().is_summary(parent));
    assert_eq!(session.outline().unwrap().children(parent), children);
    let mut after = snapshot(&session);
    after.selection = before.selection;
    assert_eq!(after, before);
}

#[test]
fn restore_into_a_collapsed_parent_expands_it() {
    let (mut session, _) = outline_session(N);
    let task = leaves(&session)
        .into_iter()
        .find(|&t| session.outline().unwrap().parent(t).is_some())
        .unwrap();
    let parent = session.outline().unwrap().parent(task).unwrap();
    session.select(task.0);
    session.delete_selected().unwrap();
    assert!(session.toggle(parent));
    assert!(session.is_collapsed(parent));
    assert_eq!(session.undo(), Ok(true));
    assert!(!session.is_collapsed(parent));
    assert_eq!(session.selection().map(|s| s.task), Some(task.0));
    assert!(session.timeline().row_of_task(task.0).is_some());
    assert_model(&session);
}

#[test]
fn structural_edits_are_refused_during_a_press_and_without_an_outline() {
    let (mut session, _) = outline_session(N);
    let task = leaves(&session)[0];
    let row = session.timeline().row_of_task(task.0).unwrap();
    let dates = session.solution().dates[task.0 as usize];
    session.fit(dates.es as f64, dates.ef.max(dates.es + 4_000) as f64);
    session.scroll_to_row(row);
    let v = &session.timeline().viewport;
    let (x, y) = (
        v.x_of(dates.es as f64) + 2.0,
        v.row_top(row) + v.row_height / 2.0,
    );
    session.pointer_down(x, y, false);
    let before = snapshot(&session);
    assert_eq!(session.apply(Edit::Delete { task }), Err(EditError::Busy));
    assert_eq!(snapshot(&session), before);
    session.pointer_up();

    let (mut flat, _) = demo_session(N);
    let error = flat.apply(Edit::Delete { task: TaskId(3) }).unwrap_err();
    assert_eq!(error, EditError::NoOutline);
    assert_eq!(error.to_string(), "this schedule has no outline");
    assert!(!flat.schedule().is_deleted(TaskId(3)));
}

#[test]
fn delete_and_undo_work_under_critical_only() {
    let (mut session, _) = outline_session(N);
    assert!(session.set_critical_only(true));
    let task = session
        .timeline()
        .order()
        .iter()
        .copied()
        .find(|&t| !session.outline().unwrap().is_summary(TaskId(t)))
        .unwrap();
    session.select(task);
    session.delete_selected().unwrap();
    assert_model(&session);
    assert!(session.undo().unwrap());
    assert_model(&session);
    assert_eq!(session.selection().map(|s| s.task), Some(task));
}

#[test]
fn a_deleted_task_is_unknown_to_every_other_edit() {
    let (mut session, _) = outline_session(N);
    let task = linked_leaf(&session);
    session.edit(Edit::Delete { task }).unwrap();
    let unknown = EditError::Schedule(ScheduleError::UnknownTask(task));
    for edit in [
        Edit::SetDuration { task, minutes: 100 },
        Edit::Rename {
            task,
            name: "x".into(),
        },
        Edit::Move { task, snet: None },
        Edit::Delete { task },
    ] {
        let before = snapshot(&session);
        assert_eq!(
            session.apply(edit.clone()),
            Err(unknown.clone()),
            "{edit:?}"
        );
        assert_eq!(snapshot(&session), before);
    }
    assert!(session.task_details(task.0).is_none());
    assert!(!session.go_to(task.0));
    session.select(task.0);
    assert!(session.selection().is_none());
}

#[test]
fn restore_refuses_a_bad_place() {
    let (mut session, _) = outline_session(N);
    let task = linked_leaf(&session);
    let live = leaves(&session).into_iter().find(|&t| t != task).unwrap();
    session.edit(Edit::Delete { task }).unwrap();
    let before = snapshot(&session);
    let sibling_of_nobody = Place {
        parent: None,
        after: Some(live),
    };
    for place in [
        sibling_of_nobody,
        Place {
            parent: Some(task),
            after: None,
        },
        Place {
            parent: Some(TaskId(u32::MAX)),
            after: None,
        },
    ] {
        assert_eq!(
            session.apply(Edit::Restore { task, place }),
            Err(EditError::BadPlace)
        );
    }
    assert_eq!(
        session.apply(Edit::Restore {
            task: live,
            place: Place {
                parent: None,
                after: None
            }
        }),
        Err(EditError::BadPlace)
    );
    assert_eq!(snapshot(&session), before);
}

#[test]
fn random_delete_undo_redo_keeps_the_model() {
    let (mut session, _) = outline_session(N);
    let mut rng = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move |bound: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % bound as u64) as usize
    };
    for step in 0..300 {
        match next(4) {
            0 | 1 => {
                let rows = session.timeline().order().to_vec();
                if let Some(&task) = rows.get(next(rows.len().max(1))) {
                    session.select(task);
                    let outline = session.outline().unwrap();
                    let summary = outline.is_summary(TaskId(task));
                    let result = session.delete_selected();
                    assert_eq!(result.is_err(), summary, "step {step}");
                }
            }
            2 => {
                session.undo().unwrap();
            }
            _ => {
                session.redo().unwrap();
            }
        }
        assert_model(&session);
    }
    while session.undo().unwrap() {}
    assert_eq!(session.task_count(), N);
    let dead = (0..N as u32)
        .filter(|&t| session.schedule().is_deleted(TaskId(t)))
        .count();
    assert_eq!(dead, 0);
    let _ = DepId(0);
}

/// One delete on the 50,000-task stress schedule, wall clock. Run:
/// `cargo test -p app --release --test it -- --ignored --nocapture structure::delete_time_on_the_stress`
#[test]
#[ignore = "a measurement, not a check"]
fn delete_time_on_the_stress_schedule() {
    let (mut session, _) = common::rooted_stress_session(50_000);
    let task = TaskId(25_000);
    session.select(task.0);
    let mut times = Vec::new();
    for _ in 0..7 {
        let t0 = std::time::Instant::now();
        session.edit(Edit::Delete { task }).expect("a leaf deletes");
        times.push(t0.elapsed().as_secs_f64() * 1e3);
        session.undo().expect("undo");
    }
    times.sort_by(f64::total_cmp);
    println!(
        "delete, 50k tasks: min {:.1} ms, median {:.1} ms",
        times[0], times[3]
    );
}
