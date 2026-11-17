//! A duration edit whose full-compute fallback fails changes nothing (gs#58 S7, amendment A1 app
//! half). The engine has its own test that the failing fallback leaves the `Solution` as it was.
//! This one holds the session to the same: `Session::edit(SetDuration)` returns `Err`, the task's
//! duration goes back, the solution, summaries, history and revision are as before, and once the
//! fault is gone the same edit applies.
//!
//! Each of the four fallback reasons is forced with the engine's `test-hooks` seams: the limit
//! override (`LIMITS`) lowers the cone's limits, and `FAIL_FALLBACK` makes the full compute of one
//! reason fail. The engine's `LAST` seam reports the reason the call fell back for, so a case
//! whose edit did not reach its reason fails here instead of passing for another one.

use crate::common;

use app::{Session, edit::Edit, edit::EditError, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::test_hooks::{FAIL_FALLBACK, LAST, LIMITS, Limits, MAX_INSPECTIONS};
use engine::{
    DAY, Dependency, FallbackReason, LinkType, Minutes, Schedule, ScheduleError, Task, TaskId,
};
use timeline::Timeline;

// Ids run in outline preorder. Three groups: a chain A, B, C; one long task X that holds the
// project finish; and Q, R with Q before R.
const A: TaskId = TaskId(1);
const B: TaskId = TaskId(2);
const C: TaskId = TaskId(3);
const X: TaskId = TaskId(5);
const Q: TaskId = TaskId(7);
const R: TaskId = TaskId(8);

/// Limits that no edit here reaches, the baseline of every call that is not forcing a limit.
const WIDE: Limits = Limits {
    admitted: 100,
    inspections: MAX_INSPECTIONS,
};

/// Puts the seams back when a test ends, also when an assertion fails, so one test's fault never
/// reaches the next test that runs on the same thread.
struct Seams;

impl Seams {
    fn new(limits: Limits) -> Seams {
        LIMITS.set(Some(limits));
        FAIL_FALLBACK.set(None);
        LAST.set(None);
        Seams
    }
}

impl Drop for Seams {
    fn drop(&mut self) {
        LIMITS.set(None);
        FAIL_FALLBACK.set(None);
        LAST.set(None);
    }
}

fn fs(pred: TaskId, succ: TaskId) -> Dependency {
    Dependency {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    }
}

/// Nine tasks on a Monday-to-Friday calendar: group "One" holds A, B and C in a chain of 2 days
/// each; "Two" holds X, 20 days long, which ends last; "Three" holds Q then R, 1 day each. Every
/// group is a summary, so each edit below moves a summary span.
fn fixture() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    let mut meta: Vec<TaskMeta> = Vec::new();
    let rows: [(&str, Minutes, Option<u32>); 9] = [
        ("One", 0, None),
        ("A", 2 * DAY, Some(0)),
        ("B", 2 * DAY, Some(0)),
        ("C", 2 * DAY, Some(0)),
        ("Two", 0, None),
        ("X", 20 * DAY, Some(4)),
        ("Three", 0, None),
        ("Q", DAY, Some(6)),
        ("R", DAY, Some(6)),
    ];
    for (name, duration, parent) in rows {
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
            .expect("a task");
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: parent.map(TaskId),
            outline_level: if parent.is_some() { 2 } else { 1 },
            code: None,
            ..TaskMeta::default()
        });
    }
    for (pred, succ) in [(A, B), (B, C), (Q, R)] {
        schedule
            .add_dependency(fs(pred, succ))
            .expect("a fixture link");
    }
    let solution = schedule.compute().expect("acyclic");
    let wbs: Vec<TaskId> = (0..9).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order = timeline::order_by_start(&solution.dates);
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

fn duration(session: &Session, task: TaskId) -> Minutes {
    session.schedule().tasks()[task.0 as usize].duration
}

fn set(task: TaskId, minutes: Minutes) -> Edit {
    Edit::SetDuration { task, minutes }
}

/// A session with an undo entry and a redo entry, so a failed edit that touched either stack
/// shows. Two edits through the cone, then one undone.
fn with_history() -> Session {
    let mut session = fixture();
    for days in [3, 4] {
        session
            .edit(set(C, days * DAY))
            .expect("a cone edit applies");
    }
    assert!(session.undo().expect("undo"));
    assert_eq!(session.history_len(), (1, 1), "the history to protect");
    session
}

/// One reason: how to force it, and an edit that reaches it.
struct Case {
    reason: FallbackReason,
    limits: Limits,
    task: TaskId,
    days: i64,
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "each call builds its Case in place and `run` takes it apart"
)]
fn run(case: Case) {
    let Case {
        reason,
        limits,
        task,
        days,
    } = case;
    let _seams = Seams::new(limits);
    let mut session = with_history();
    let minutes = duration(&session, task) + days * DAY;
    let original = duration(&session, task);
    let (tasks, solution, summaries, history, revision, stamp) = (
        session.schedule().tasks().to_vec(),
        session.solution().clone(),
        session.timeline().summaries().clone(),
        session.history_len(),
        session.revision(),
        session.stamp(),
    );

    FAIL_FALLBACK.set(Some(reason));
    let error = session
        .edit(set(task, minutes))
        .expect_err("the failing fallback is an error");
    assert!(
        matches!(error, EditError::Schedule(ScheduleError::Infeasible(_))),
        "{reason:?}: the fault's own error, got {error:?}"
    );
    assert_eq!(LAST.get(), Some(reason), "{reason:?}: the reason forced");

    assert_eq!(duration(&session, task), original, "{reason:?}: duration");
    assert_eq!(session.schedule().tasks(), tasks, "{reason:?}: tasks");
    assert_eq!(session.solution(), &solution, "{reason:?}: solution");
    assert_eq!(
        session.timeline().summaries(),
        &summaries,
        "{reason:?}: summaries"
    );
    assert_eq!(session.history_len(), history, "{reason:?}: history");
    assert_eq!(session.revision(), revision, "{reason:?}: revision");
    assert_eq!(session.stamp(), stamp, "{reason:?}: stamp");

    // The fault gone, the same edit applies by the same fallback, and the session matches a full
    // solve and moves a summary: the case edit is not a no-op.
    FAIL_FALLBACK.set(None);
    LAST.set(None);
    session
        .edit(set(task, minutes))
        .expect("the same edit applies without the fault");
    assert_eq!(LAST.get(), Some(reason), "{reason:?}: forced again");
    assert_eq!(duration(&session, task), minutes, "{reason:?}: applied");
    let fresh = session.schedule().clone().compute().expect("acyclic");
    assert_eq!(session.solution(), &fresh, "{reason:?}: full compute");
    assert_ne!(
        session.timeline().summaries(),
        &summaries,
        "{reason:?}: the edit moves a summary"
    );
    assert_eq!(session.history_len(), (history.0 + 1, 0), "{reason:?}");
    assert!(session.undo().expect("undo"));
    assert_eq!(duration(&session, task), original, "{reason:?}: undone");
    assert_eq!(session.solution(), &solution, "{reason:?}: undone solution");
}

/// B grows, so its successor C moves: the forward pass would admit a second task, over a limit of 1.
#[test]
fn a_failed_forward_limit_fallback_leaves_the_session_unchanged() {
    run(Case {
        reason: FallbackReason::ForwardLimit,
        limits: Limits {
            admitted: 1,
            ..WIDE
        },
        task: B,
        days: 1,
    });
}

/// R grows inside the finish, so nothing moves forward and the backward pass would admit its
/// predecessor Q, the second task over a limit of 1.
#[test]
fn a_failed_backward_limit_fallback_leaves_the_session_unchanged() {
    run(Case {
        reason: FallbackReason::BackwardLimit,
        limits: Limits {
            admitted: 1,
            ..WIDE
        },
        task: R,
        days: 1,
    });
}

/// R has one predecessor, so the first inspection is over a limit of none.
#[test]
fn a_failed_edge_limit_fallback_leaves_the_session_unchanged() {
    run(Case {
        reason: FallbackReason::EdgeLimit,
        limits: Limits {
            inspections: 0,
            ..WIDE
        },
        task: R,
        days: 1,
    });
}

/// X ends the project, so a longer X moves the finish, with limits no edit here reaches.
#[test]
fn a_failed_finish_move_fallback_leaves_the_session_unchanged() {
    run(Case {
        reason: FallbackReason::FinishMove,
        limits: WIDE,
        task: X,
        days: 10,
    });
}
