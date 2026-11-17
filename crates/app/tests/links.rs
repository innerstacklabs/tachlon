//! Add, remove and restore a link through `Session::edit` (gs#345, gs#49 PR 3): the dates match a
//! reference solve, undo and redo are exact and keep the link's id, and a refusal changes nothing.

use crate::common;

use app::{
    Place, RowFilter, Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
    outline::Outline,
};
use common::{clock, setup};
use engine::{
    Calendar, DepId, Dependency, LinkType, Minutes, Schedule, ScheduleError, Task, TaskId,
};
use timeline::Timeline;

const HOUR: Minutes = 60;

/// A 24/7 schedule of `tasks` (name, parent, duration in minutes), listed in outline preorder so
/// that the id of a task is its index, with an outline and a finish-to-start link per `links`.
fn build(tasks: &[(&str, Option<u32>, Minutes)], links: &[(u32, u32)]) -> Session {
    build_with(tasks, links, |_| {})
}

/// [`build`], with `seed` changing the task meta first, as an imported file brings activity IDs.
pub(crate) fn build_with(
    tasks: &[(&str, Option<u32>, Minutes)],
    links: &[(u32, u32)],
    seed: impl FnOnce(&mut Vec<TaskMeta>),
) -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    let mut meta: Vec<TaskMeta> = Vec::new();
    for &(name, parent, duration) in tasks {
        let task = Task {
            duration,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
        let outline_level = parent.map_or(1, |p| meta[p as usize].outline_level + 1);
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: parent.map(TaskId),
            outline_level,
            code: None,
            ..TaskMeta::default()
        });
    }
    for &(pred, succ) in links {
        schedule
            .add_dependency(fs(pred, succ))
            .expect("a fixture link");
    }
    seed(&mut meta);
    let solution = schedule.compute().expect("acyclic");
    let wbs: Vec<TaskId> = (0..tasks.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order = (0..tasks.len() as u32).collect();
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

fn fs(pred: u32, succ: u32) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

fn add(pred: u32, succ: u32) -> Edit {
    Edit::AddLink {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind: LinkType::FS,
        lag: 0,
    }
}

/// Four hour-long leaves: two chains, `A -> B` and `C -> D`.
fn chains() -> Session {
    build(
        &[
            ("A", None, HOUR),
            ("B", None, HOUR),
            ("C", None, 3 * HOUR),
            ("D", None, HOUR),
        ],
        &[(0, 1), (2, 3)],
    )
}

/// A schedule built from scratch with the dependencies in force: what `compute` must agree with.
fn reference(session: &Session) -> engine::Solution {
    let live = session.schedule();
    let mut fresh = Schedule::new();
    for calendar in live.calendars() {
        fresh.add_calendar(calendar.clone());
    }
    for task in live.tasks() {
        fresh.add_task(task.clone()).expect("a task");
    }
    for (_, dependency) in live.live_dependencies() {
        fresh.add_dependency(*dependency).expect("a link");
    }
    fresh.compute().expect("acyclic")
}

fn assert_matches_reference(session: &Session) {
    let fresh = reference(session);
    assert_eq!(session.solution().dates, fresh.dates);
    assert_eq!(session.solution().project_finish, fresh.project_finish);
}

/// What a refused edit must leave alone.
fn state(session: &Session) -> impl PartialEq + std::fmt::Debug + use<> {
    (
        session.revision(),
        session.history_len(),
        session.solution().dates.clone(),
        session.schedule().dependencies().len(),
        session.schedule().live_dependencies().count(),
        session.timeline().order().to_vec(),
    )
}

fn refused(session: &mut Session, edit: Edit) -> EditError {
    let before = state(session);
    let error = session.edit(edit).expect_err("refused");
    assert_eq!(state(session), before, "{error}");
    error
}

#[test]
fn adding_a_link_re_solves_like_a_fresh_schedule() {
    let mut session = chains();
    session.edit(add(1, 2)).expect("adds");
    // C waits for B now: it starts at the end of B's two hours.
    assert_eq!(
        session.solution().dates[2].es,
        session.solution().dates[1].ef
    );
    assert_matches_reference(&session);
    assert_eq!(session.history_len(), (1, 0));
    assert_eq!(session.links().len(), 3);
}

#[test]
fn removing_and_restoring_a_link_re_solve_like_a_fresh_schedule() {
    let mut session = chains();
    let original = session.solution().dates.clone();
    session
        .edit(Edit::RemoveLink { dep: DepId(0) })
        .expect("removes");
    assert_matches_reference(&session);
    assert_eq!(session.solution().dates[1].es, 0, "B is free of A");
    assert_eq!(session.links().len(), 1);
    session
        .edit(Edit::RestoreLink { dep: DepId(0) })
        .expect("restores");
    assert_matches_reference(&session);
    assert_eq!(session.solution().dates, original);
}

#[test]
fn apply_returns_the_inverse_of_each_link_edit() {
    let mut session = chains();
    let remove = session.apply(add(1, 2)).expect("adds");
    assert_eq!(remove, Edit::RemoveLink { dep: DepId(2) });
    let restore = session.apply(remove).expect("removes");
    assert_eq!(restore, Edit::RestoreLink { dep: DepId(2) });
    let remove = session.apply(restore).expect("restores");
    assert_eq!(remove, Edit::RemoveLink { dep: DepId(2) });
}

#[test]
fn undo_returns_to_the_original_solution_and_redo_keeps_the_link_id() {
    let mut session = chains();
    let original = session.revision();
    let dates = session.solution().dates.clone();
    session.edit(add(1, 2)).expect("adds");
    let linked = session.solution().dates.clone();
    assert_ne!(linked, dates);
    let live_ids = |s: &Session| -> Vec<DepId> {
        s.schedule().live_dependencies().map(|(id, _)| id).collect()
    };
    assert_eq!(live_ids(&session), [DepId(0), DepId(1), DepId(2)]);

    assert!(session.undo().expect("undoes"));
    assert_eq!(session.solution().dates, dates);
    assert_matches_reference(&session);
    assert_eq!(live_ids(&session), [DepId(0), DepId(1)]);
    assert!(session.revision() > original, "undo is an edit too");

    assert!(session.redo().expect("redoes"));
    assert_eq!(session.solution().dates, linked);
    assert_eq!(live_ids(&session), [DepId(0), DepId(1), DepId(2)]);
    assert_eq!(session.schedule().dependencies().len(), 3, "same slot");
}

#[test]
fn undo_and_redo_of_a_remove_are_exact() {
    let mut session = chains();
    let dates = session.solution().dates.clone();
    session
        .edit(Edit::RemoveLink { dep: DepId(1) })
        .expect("removes");
    let removed = session.solution().dates.clone();
    assert!(session.undo().expect("undoes"));
    assert_eq!(session.solution().dates, dates);
    assert!(session.redo().expect("redoes"));
    assert_eq!(session.solution().dates, removed);
    assert_matches_reference(&session);
    assert!(session.undo().expect("undoes"));
    assert_eq!(session.solution().dates, dates);
}

#[test]
fn a_fresh_add_after_an_undo_takes_a_new_slot() {
    let mut session = chains();
    session.edit(add(1, 2)).expect("adds");
    session.undo().expect("undoes");
    session.edit(add(1, 2)).expect("adds again");
    assert_eq!(session.schedule().dependencies().len(), 4);
    assert_eq!(session.history_len(), (1, 0), "the redo stack is gone");
    assert!(session.schedule().is_dependency_live(DepId(3)));
    assert!(!session.schedule().is_dependency_live(DepId(2)));
}

#[test]
fn a_loop_is_refused_with_its_path() {
    let mut session = build(
        &[("a", None, HOUR), ("b", None, HOUR), ("c", None, HOUR)],
        &[(0, 1), (1, 2)],
    );
    let error = refused(&mut session, add(2, 0));
    assert_eq!(
        error.to_string(),
        "\"c\" to \"a\" would make a loop: a → b → c → a."
    );
    let EditError::LinkCycle { path, .. } = error else {
        panic!("a loop");
    };
    assert_eq!(path, [TaskId(2), TaskId(0), TaskId(1)]);
}

#[test]
fn a_two_task_loop_names_both() {
    let mut session = build(&[("a", None, HOUR), ("b", None, HOUR)], &[(0, 1)]);
    let error = refused(&mut session, add(1, 0));
    assert_eq!(
        error.to_string(),
        "\"b\" to \"a\" would make a loop: a → b → a."
    );
}

#[test]
fn a_long_loop_is_elided_past_five_tasks() {
    let names: Vec<String> = (0..12).map(|i| format!("T{i}")).collect();
    let tasks: Vec<(&str, Option<u32>, Minutes)> =
        names.iter().map(|n| (n.as_str(), None, HOUR)).collect();
    let links: Vec<(u32, u32)> = (0..11).map(|i| (i, i + 1)).collect();
    let mut session = build(&tasks, &links);
    let error = refused(&mut session, add(11, 0));
    assert_eq!(
        error.to_string(),
        "\"T11\" to \"T0\" would make a loop: T0 → T1 → T2 → … (12 tasks) → T11 → T0."
    );
}

#[test]
fn a_loop_of_five_is_named_in_full_and_of_six_is_elided() {
    for (n, expected) in [
        (5, "T0 → T1 → T2 → T3 → T4 → T0"),
        (6, "T0 → T1 → T2 → … (6 tasks) → T5 → T0"),
    ] {
        let names: Vec<String> = (0..n).map(|i| format!("T{i}")).collect();
        let tasks: Vec<(&str, Option<u32>, Minutes)> =
            names.iter().map(|n| (n.as_str(), None, HOUR)).collect();
        let links: Vec<(u32, u32)> = (0..n as u32 - 1).map(|i| (i, i + 1)).collect();
        let mut session = build(&tasks, &links);
        let error = refused(&mut session, add(n as u32 - 1, 0));
        assert!(
            error.to_string().ends_with(&format!("{expected}.")),
            "{error}"
        );
    }
}

#[test]
fn a_task_with_an_activity_id_is_named_after_it() {
    let mut session = build_with(&[("a", None, HOUR), ("b", None, HOUR)], &[(0, 1)], |meta| {
        meta[0].code = Some("A1010".to_string());
    });
    let error = refused(&mut session, add(1, 0));
    assert_eq!(
        error.to_string(),
        "\"b\" to \"A1010 a\" would make a loop: A1010 a → b → A1010 a."
    );
}

#[test]
fn a_summary_is_refused_at_either_end() {
    let mut session = build(
        &[
            ("Area", None, HOUR),
            ("Dig", Some(0), HOUR),
            ("Pour", Some(0), HOUR),
            ("Cure", None, HOUR),
        ],
        &[],
    );
    for edit in [add(0, 3), add(3, 0)] {
        let error = refused(&mut session, edit);
        assert_eq!(
            error,
            EditError::LinkToSummary {
                name: "Area".to_string()
            }
        );
        assert_eq!(
            error.to_string(),
            "\"Area\" is a summary. Link its sub-tasks instead."
        );
    }
    // Its sub-tasks take links.
    session
        .edit(add(1, 3))
        .expect("a leaf under a summary links");
}

#[test]
fn a_second_live_link_on_a_pair_is_refused_whatever_its_type() {
    let mut session = chains();
    for kind in [LinkType::FS, LinkType::SS, LinkType::FF] {
        let error = refused(
            &mut session,
            Edit::AddLink {
                pred: TaskId(0),
                succ: TaskId(1),
                kind,
                lag: 30,
            },
        );
        assert_eq!(
            error,
            EditError::AlreadyLinked {
                pred: TaskId(0),
                succ: TaskId(1),
                pred_name: "A".to_string(),
                succ_name: "B".to_string(),
            }
        );
        assert_eq!(error.to_string(), "\"A\" is already linked to \"B\".");
    }
}

#[test]
fn unknown_and_deleted_ends_and_links_are_refused() {
    let mut session = chains();
    let unknown = |task| EditError::Schedule(ScheduleError::UnknownTask(TaskId(task)));
    assert_eq!(refused(&mut session, add(0, 99)), unknown(99));
    assert_eq!(refused(&mut session, add(99, 0)), unknown(99));
    assert_eq!(
        refused(&mut session, add(0, 0)),
        EditError::Schedule(ScheduleError::SelfDependency(TaskId(0)))
    );
    assert_eq!(
        refused(&mut session, Edit::RemoveLink { dep: DepId(9) }),
        EditError::UnknownLink(DepId(9))
    );
    assert_eq!(
        refused(&mut session, Edit::RestoreLink { dep: DepId(9) }),
        EditError::UnknownLink(DepId(9))
    );
    // A live link is not removed, so there is nothing to restore.
    assert!(matches!(
        refused(&mut session, Edit::RestoreLink { dep: DepId(0) }),
        EditError::AlreadyLinked { .. }
    ));
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("deletes");
    assert_eq!(refused(&mut session, add(0, 1)), unknown(1));
    assert_eq!(refused(&mut session, add(1, 0)), unknown(1));
    // The link to the deleted task is dead: it cannot be removed, or restored.
    assert_eq!(
        refused(&mut session, Edit::RemoveLink { dep: DepId(0) }),
        EditError::UnknownLink(DepId(0))
    );
    assert!(matches!(
        refused(&mut session, Edit::RestoreLink { dep: DepId(0) }),
        EditError::AlreadyLinked { .. } | EditError::Schedule(ScheduleError::UnknownTask(_))
    ));
}

#[test]
fn a_remove_twice_is_refused_and_a_stale_restore_is_checked() {
    let mut session = chains();
    session
        .edit(Edit::RemoveLink { dep: DepId(0) })
        .expect("removes");
    assert_eq!(
        refused(&mut session, Edit::RemoveLink { dep: DepId(0) }),
        EditError::UnknownLink(DepId(0))
    );
    // `apply` is public and a host may apply out of order: the way back is a loop by now.
    session.edit(add(1, 0)).expect("the other way is free");
    let error = refused(&mut session, Edit::RestoreLink { dep: DepId(0) });
    assert_eq!(
        error.to_string(),
        "\"A\" to \"B\" would make a loop: B → A → B."
    );
    // And a duplicate of what was added since.
    session.edit(add(0, 1)).unwrap_err();
}

#[test]
fn a_restore_onto_a_summary_is_refused() {
    let mut session = build(&[("L", None, HOUR), ("X", None, HOUR)], &[(0, 1)]);
    session
        .edit(Edit::RemoveLink { dep: DepId(0) })
        .expect("removes");
    // A removed link no longer keeps L a leaf: it takes a child.
    session
        .edit(Edit::Insert {
            place: Place {
                parent: Some(TaskId(0)),
                after: None,
            },
            name: "Child".to_string(),
        })
        .expect("a removed link does not touch L");
    let error = refused(&mut session, Edit::RestoreLink { dep: DepId(0) });
    assert_eq!(
        error,
        EditError::LinkToSummary {
            name: "L".to_string()
        }
    );
}

#[test]
fn undo_after_deleting_the_only_child_survives_a_removed_link_on_the_parent() {
    let mut session = build(
        &[("P", None, HOUR), ("C", Some(0), HOUR), ("X", None, HOUR)],
        &[],
    );
    let dates = session.solution().dates.clone();
    session
        .edit(Edit::Delete { task: TaskId(1) })
        .expect("deletes");
    // P has no child left, so it is a leaf and takes a link.
    session.edit(add(0, 2)).expect("links the emptied parent");
    assert!(session.undo().expect("removes the link"));
    // The outline is rebuilt with P a summary again: the removed link must not stop it.
    assert!(session.undo().expect("restores the child"));
    assert_eq!(session.solution().dates, dates);
    assert!(session.outline().unwrap().is_summary(TaskId(0)));
    assert!(session.redo().expect("deletes again"));
    assert!(session.redo().expect("links again"));
    assert!(session.schedule().is_dependency_live(DepId(0)));
    assert!(session.undo().expect("removes the link"));
    assert!(session.undo().expect("restores the child"));
}

#[test]
fn a_dead_link_does_not_keep_a_parent_from_taking_a_child() {
    // The link is live, so a leaf with it cannot become a summary; removed, it can.
    let mut session = build(&[("L", None, HOUR), ("X", None, HOUR)], &[(0, 1)]);
    let place = Place {
        parent: Some(TaskId(0)),
        after: None,
    };
    let insert = Edit::Insert {
        place,
        name: "Child".to_string(),
    };
    assert!(matches!(
        refused(&mut session, insert.clone()),
        EditError::Outline(_)
    ));
    session
        .edit(Edit::RemoveLink { dep: DepId(0) })
        .expect("removes");
    session.edit(insert).expect("inserts");
}

#[test]
fn a_summary_highlight_leaves_out_a_removed_link() {
    let mut session = build(
        &[
            ("Area", None, HOUR),
            ("Dig", Some(0), HOUR),
            ("Pour", Some(0), HOUR),
        ],
        &[(1, 2)],
    );
    session.select(0);
    assert!(session.highlight().unwrap().has_edge(1, 2));
    session
        .edit(Edit::RemoveLink { dep: DepId(0) })
        .expect("removes");
    session.select(0);
    assert!(!session.highlight().unwrap().has_edge(1, 2));
}

#[test]
fn a_link_edit_under_the_critical_filter_refreshes_the_rows() {
    let mut session = build(
        &[("A", None, HOUR), ("B", None, 2 * HOUR), ("C", None, HOUR)],
        &[],
    );
    assert!(session.set_critical_only(true));
    assert_eq!(session.row_filter(), RowFilter::Critical);
    assert_eq!(session.timeline().order(), [1]);
    assert_eq!(session.links().len(), 0);

    let stamp = session.stamp();
    session.edit(add(1, 2)).expect("adds");
    // C now ends the project with B: both are critical, A is not.
    assert_eq!(session.timeline().order(), [1, 2]);
    assert_eq!(session.links().len(), 1);
    assert_ne!(session.stamp(), stamp);
    assert_matches_reference(&session);

    assert!(session.undo().expect("undoes"));
    assert_eq!(session.timeline().order(), [1]);
    assert_eq!(session.links().len(), 0);
    assert!(session.redo().expect("redoes"));
    assert_eq!(session.timeline().order(), [1, 2]);
}

#[test]
fn a_busy_session_refuses_every_link_edit() {
    let (mut session, _, _, point) = setup();
    session.pointer_down(point.0, point.1, false);
    let before = state(&session);
    for edit in [
        add(0, 1),
        Edit::RemoveLink { dep: DepId(0) },
        Edit::RestoreLink { dep: DepId(0) },
    ] {
        assert_eq!(session.edit(edit).unwrap_err(), EditError::Busy);
    }
    assert_eq!(state(&session), before);
    session.pointer_up();
}

/// The cost of one add-link edit on 50,000 tasks: `cargo test --release -p app --test it -- --ignored
/// --nocapture links::timed_add_link_edit_at_50k`.
#[test]
#[ignore = "timing: run with --release --ignored --nocapture"]
fn timed_add_link_edit_at_50k() {
    let (mut schedule, solution, order, meta) = app::demo(50_000);
    let topo = schedule.topological_order().expect("orders");
    let n = topo.len();
    let (pred, succ) = (topo[n / 2], topo[n / 2 + n / 10]);
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    let mut session = Session::new(schedule, solution, timeline, meta, clock);
    let t = std::time::Instant::now();
    session
        .edit(add(pred.0, succ.0))
        .expect("a forward link makes no loop");
    let edit_ms = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    assert!(session.undo().expect("undoes"));
    let undo_ms = t.elapsed().as_secs_f64() * 1e3;
    eprintln!("50k add-link edit: {edit_ms:.1} ms, its undo: {undo_ms:.1} ms");
}
