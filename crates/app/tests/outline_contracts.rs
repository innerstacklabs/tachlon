//! The six length and range `assert!`s in `outline.rs` (gs#163) stay, because no code path reaches
//! them. These tests are the proof, and they name the premise each assert rests on:
//!
//! 1. `rollup_full` and `rollup_moved` assert that `dates` and `out` have the outline's length.
//!    Premise: a `Session` only installs an outline of its own slot count, its `dates` and its
//!    summaries have that count, and no edit changes it. A delete leaves a tombstone in its slot
//!    (gs#50), so the slot count is not the live count; `Outline::new` takes the live ids in `wbs`
//!    and sizes its arrays by slot.
//! 2. `children` and `is_summary` assert that the task id is in range. Premise: every id comes
//!    from the schedule or the outline, or is checked against the slot count at the boundary.
//!
//! An insert (gs#50 PR 3) appends a slot to all four lengths at once; test 3 then covers it.

use crate::common;

use app::{
    Place, Session, Siblings,
    edit::Edit,
    meta::TaskMeta,
    outline::{Outline, OutlineError, demo_layout},
};
use common::{clock, outline_session};
use engine::{Calendar, DepId, Limit, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const N: usize = 300;

fn schedule_with_tasks(task_count: usize) -> Schedule {
    let mut schedule = Schedule::new();
    let day = vec![(480, 720), (780, 1020)];
    let calendar = schedule.add_calendar(
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
        .expect("valid standard calendar"),
    );
    for _ in 0..task_count {
        schedule
            .add_task(Task {
                duration: 0,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule
}

/// The three lengths the rollup asserts compare, and the task count they must equal.
fn lengths(session: &Session) -> [usize; 4] {
    let outline = session.outline().expect("test session has an outline");
    [
        outline.len(),
        session.solution().dates.len(),
        session.timeline().summaries().len(),
        session.schedule().tasks().len(),
    ]
}

fn assert_lengths_agree(session: &Session, when: &str) {
    let lengths = lengths(session);
    assert!(
        lengths.iter().all(|&len| len == lengths[0]),
        "outline, dates, summaries and task count must agree {when}: {lengths:?}"
    );
}

#[test]
fn a_session_refuses_an_outline_of_another_length() {
    // The rollup asserts compare against the outline's length, and `with_outline` is the only way in.
    let layout = demo_layout(N);
    let small = schedule_with_tasks(3);
    let meta: Vec<TaskMeta> = (0..3)
        .map(|_| TaskMeta {
            name: "task".to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        })
        .collect();
    let wbs: Vec<_> = (0..3).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &small).expect("valid forest");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    );

    assert!(matches!(
        session.with_outline(outline),
        Err(OutlineError::WrongLength)
    ));
}

#[test]
fn an_outline_is_built_only_for_matching_lengths() {
    let layout = demo_layout(N);
    let short = &layout.wbs[..N - 1];
    assert_eq!(
        Outline::new(&layout.meta, short, &layout.schedule).err(),
        Some(OutlineError::WrongLength)
    );
    assert_eq!(
        Outline::new(&layout.meta[..N - 1], &layout.wbs, &layout.schedule).err(),
        Some(OutlineError::WrongLength)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test runs every kind of edit against the rollup"
)]
fn no_edit_changes_the_lengths_the_rollup_compares() {
    let (mut session, _) = outline_session(N);
    assert_lengths_agree(&session, "after with_outline");
    let leaf = session
        .outline()
        .unwrap()
        .preorder()
        .iter()
        .copied()
        .find(|&task| !session.outline().unwrap().is_summary(task))
        .expect("the demo has a leaf");

    let outline = session.outline().unwrap();
    let parent = outline.parent(leaf);
    let after = parent
        .map_or(outline.roots(), |parent| outline.children(parent))
        .iter()
        .position(|&sibling| sibling == leaf)
        .and_then(|at| at.checked_sub(1))
        .map(|at| parent.map_or(outline.roots(), |p| outline.children(p))[at]);
    // Two leaves, the earlier in topological order first, with no link between them.
    let mut schedule = session.schedule().clone();
    let leaves: Vec<TaskId> = schedule
        .topological_order()
        .expect("orders")
        .into_iter()
        .filter(|&task| task != leaf && !session.outline().unwrap().is_summary(task))
        .collect();
    let linked = |a: TaskId, b: TaskId| {
        session
            .schedule()
            .dependencies()
            .iter()
            .any(|d| (d.pred, d.succ) == (a, b))
    };
    let (pred, succ) = (1..leaves.len())
        .flat_map(|j| (0..j).map(move |i| (i, j)))
        .map(|(i, j)| (leaves[i], leaves[j]))
        .find(|&(a, b)| !linked(a, b))
        .expect("an unlinked pair");
    let dep = DepId(session.schedule().dependencies().len() as u32);
    // One value of every Edit variant. A variant added later fails to compile in the match
    // below, and this test is where its effect on the lengths shows first.
    let edits = [
        Edit::SetDuration {
            task: leaf,
            minutes: 4_000,
        },
        Edit::Move {
            task: leaf,
            snet: None,
        },
        Edit::Rename {
            task: leaf,
            name: "renamed".to_string(),
        },
        Edit::Delete { task: leaf },
        Edit::Restore {
            task: leaf,
            place: Place { parent, after },
        },
        Edit::Insert {
            place: Place {
                parent,
                after: Some(leaf),
            },
            name: "inserted".to_string(),
        },
        Edit::Reparent {
            task: leaf,
            place: Place {
                parent: None,
                after: None,
            },
            siblings: Siblings::Keep,
        },
        Edit::AddLink {
            pred,
            succ,
            kind: LinkType::FS,
            lag: 0,
        },
        Edit::RemoveLink { dep },
        Edit::RestoreLink { dep },
        Edit::SetLink {
            dep,
            kind: LinkType::SS,
            lag: 60,
        },
        Edit::SetBaseline { baseline: None },
        Edit::ClearBaseline,
        Edit::SetDataDate {
            date: Some(2 * 1440),
        },
        Edit::SetProgress {
            task: leaf,
            actual_start: Some(2 * 1440),
            actual_finish: None,
            remaining: Some(480),
        },
        Edit::SetLimit {
            task: leaf,
            limit: Limit::default(),
        },
    ];
    for edit in edits {
        let kind = match &edit {
            Edit::Move { .. } => "Move",
            Edit::SetDuration { .. } => "SetDuration",
            Edit::Rename { .. } => "Rename",
            Edit::Delete { .. } => "Delete",
            Edit::Restore { .. } => "Restore",
            Edit::Reparent { .. } => "Reparent",
            Edit::Insert { .. } => "Insert",
            Edit::AddLink { .. } => "AddLink",
            Edit::RemoveLink { .. } => "RemoveLink",
            Edit::RestoreLink { .. } => "RestoreLink",
            Edit::SetLink { .. } => "SetLink",
            Edit::SetBaseline { .. } => "SetBaseline",
            Edit::ClearBaseline => "ClearBaseline",
            Edit::SetDataDate { .. } => "SetDataDate",
            Edit::SetProgress { .. } => "SetProgress",
            Edit::SetLimit { .. } => "SetLimit",
        };
        session.edit(edit).expect("edit applies");
        assert_lengths_agree(&session, kind);
    }
    while session.undo().expect("undo") {
        assert_lengths_agree(&session, "after undo");
    }
    while session.redo().expect("redo") {
        assert_lengths_agree(&session, "after redo");
    }
}

#[test]
fn collapse_levels_and_a_bar_drag_keep_the_lengths() {
    let (mut session, _meta) = outline_session(N);
    for level in [1, 2, 3] {
        session.collapse_to_level(level);
        assert_lengths_agree(&session, "after collapse_to_level");
    }
    session.expand_all();
    assert_lengths_agree(&session, "after expand_all");
    // A press with nothing under it, then a move and release: the path a drag takes through `rollup`.
    session.pointer_down(500.0, 200.0, false);
    session.pointer_move(540.0, 200.0);
    session.apply_drag();
    session.pointer_up();
    assert_lengths_agree(&session, "after a drag");
}

#[test]
fn an_out_of_range_task_never_reaches_the_outline() {
    // `children` and `is_summary` assert `task < len`. Every Session entry that takes an id from outside
    // checks it first; the rest take ids from the outline or the schedule, which are in range.
    let (mut session, _) = outline_session(N);
    let past = TaskId(N as u32);
    for task in [past, TaskId(N as u32 + 1), TaskId(u32::MAX)] {
        assert!(!session.toggle(task), "toggle {task:?}");
        assert!(!session.is_collapsed(task), "is_collapsed {task:?}");
        assert!(!session.go_to(task.0), "go_to {task:?}");
        session.select(task.0);
        assert!(
            session.task_details(task.0).is_none(),
            "task_details {task:?}"
        );
    }
    assert!(
        session
            .timeline()
            .order()
            .iter()
            .all(|&task| (task as usize) < N),
        "the rows hold only task ids in range"
    );
    let outline = session.outline().expect("test session has an outline");
    for &task in outline.preorder() {
        // The ids the outline hands out are valid arguments for the outline.
        let _ = outline.is_summary(task);
        assert!(
            outline
                .children(task)
                .iter()
                .all(|child| (child.0 as usize) < N)
        );
    }
    assert!(
        session
            .schedule()
            .dependencies()
            .iter()
            .all(|dep| (dep.pred.0 as usize) < N && (dep.succ.0 as usize) < N),
        "every dependency end is a task id in range"
    );
}

#[test]
fn the_outline_takes_live_ids_and_sizes_its_arrays_by_slot() {
    let layout = demo_layout(N);
    let mut schedule = layout.schedule.clone();
    let leaf = layout
        .wbs
        .iter()
        .copied()
        .find(|&t| layout.meta.iter().all(|m| m.parent != Some(t)))
        .expect("the demo has a leaf");
    schedule.delete_task(leaf).expect("a live task deletes");
    let live: Vec<TaskId> = layout.wbs.iter().copied().filter(|&t| t != leaf).collect();

    let outline = Outline::new(&layout.meta, &live, &schedule).expect("live ids make an outline");
    assert_eq!(outline.len(), N, "sized by slot count");
    assert_eq!(outline.preorder().len(), N - 1);
    assert_eq!(
        outline.parent(leaf),
        None,
        "a dead task's parent is ignored"
    );
    assert_eq!(outline.subtree(leaf), []);
    // The dead id in `wbs`, or a missing live id, is the wrong set of tasks.
    assert_eq!(
        Outline::new(&layout.meta, &layout.wbs, &schedule).err(),
        Some(OutlineError::WrongLength)
    );
    assert_eq!(
        Outline::new(&layout.meta, &live[1..], &schedule).err(),
        Some(OutlineError::WrongLength)
    );
}
