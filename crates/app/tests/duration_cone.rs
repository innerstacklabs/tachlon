//! A duration edit re-solves through the duration cone and rolls the moved summaries up (gs#380,
//! gs#58 PR 4). The oracle is a full `compute()` on a clone of the schedule after every step, and
//! a summary span rebuilt from the leaves of that fresh solution, so a stale summary or a cone
//! that disagrees with `compute()` fails the step that produced it.

use crate::common;

use app::{SEED, Session, edit::Edit, meta::TaskMeta, outline::Outline};
use common::{clock, outline_session};
use engine::{
    DAY, Dependency, DurationCone, FallbackReason, LinkType, Minutes, Rng, Schedule, Task, TaskId,
};
use timeline::{Summaries, Timeline};

/// What a full solve says, whatever the session did to get its own answer: the solution, and every
/// summary span taken from the leaf descendants of that fresh solution.
fn assert_matches_full_compute(session: &Session, step: &str) {
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the schedule is acyclic");
    assert_eq!(session.solution(), &fresh, "solution after {step}");
    let outline = session.outline().expect("test session has an outline");
    let mut expected = Summaries::new(outline.len());
    for &task in outline.preorder() {
        if !outline.is_summary(task) {
            continue;
        }
        let span = outline
            .subtree(task)
            .iter()
            .copied()
            .filter(|&descendant| !outline.is_summary(descendant))
            .map(|leaf| {
                (
                    fresh.dates[leaf.0 as usize].es,
                    fresh.dates[leaf.0 as usize].ef,
                )
            })
            .reduce(|(min_es, max_ef), (es, ef)| (min_es.min(es), max_ef.max(ef)))
            .expect("a summary has a leaf descendant");
        expected.set(task.0, Some(span));
    }
    assert_eq!(
        session.timeline().summaries(),
        &expected,
        "summary spans after {step}"
    );
}

const GROUPS: u32 = 4;
const LEAVES: u32 = 25;
/// One summary and its leaves: ids run in outline preorder, so a group is a block.
const BLOCK: u32 = LEAVES + 1;

fn summary_of(group: u32) -> TaskId {
    TaskId(group * BLOCK)
}

fn leaf(group: u32, index: u32) -> TaskId {
    TaskId(group * BLOCK + 1 + index)
}

fn fs(pred: TaskId, succ: TaskId) -> Dependency {
    Dependency {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    }
}

fn add_link(pred: TaskId, succ: TaskId) -> Edit {
    Edit::AddLink {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    }
}

/// Four summaries of 25 leaves on a Monday-to-Friday calendar. A leaf is linked to the next one
/// in runs of five, so every cone is small enough for the cone path and the runs are independent
/// until a test links them. Leaf durations are 1, 2 and 3 days in turn, except the first leaf of
/// group 0, which is 200 days long: it holds the project finish, so an edit anywhere else that
/// stays shorter leaves the finish where it is and takes the cone, not the full-compute fallback.
fn grouped() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    let mut meta: Vec<TaskMeta> = Vec::new();
    for group in 0..GROUPS {
        let summary = summary_of(group);
        let mut add = |duration: Minutes, parent: Option<TaskId>, level: u8, name: String| {
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
                name,
                parent,
                outline_level: level,
                code: None,
                ..TaskMeta::default()
            });
        };
        add(0, None, 1, format!("Group {group}"));
        for index in 0..LEAVES {
            let minutes = if (group, index) == (0, 0) {
                200 * DAY
            } else {
                (i64::from(index % 3) + 1) * 480
            };
            add(minutes, Some(summary), 2, format!("Task {group}.{index}"));
        }
    }
    for group in 0..GROUPS {
        for index in 0..LEAVES - 1 {
            if index % 5 != 4 {
                schedule
                    .add_dependency(fs(leaf(group, index), leaf(group, index + 1)))
                    .expect("a fixture link");
            }
        }
    }
    let solution = schedule.compute().expect("acyclic");
    let wbs: Vec<TaskId> = (0..GROUPS * BLOCK).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order = timeline::order_by_start(&solution.dates);
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline fits")
}

fn set(session: &mut Session, task: TaskId, minutes: Minutes) {
    session
        .edit(Edit::SetDuration { task, minutes })
        .expect("the duration applies");
}

fn duration(session: &Session, task: TaskId) -> Minutes {
    session.schedule().tasks()[task.0 as usize].duration
}

#[test]
fn a_duration_edit_rolls_up_the_summary_it_moves() {
    let mut session = grouped();
    assert_matches_full_compute(&session, "open");
    let before = session.timeline().summaries().get(summary_of(1).0);

    // The last leaf of a run has no successor: its finish is the group's latest, so its summary
    // must follow it.
    let tail = leaf(1, 24);
    let longer = duration(&session, tail) + 40 * DAY;
    set(&mut session, tail, longer);
    assert_matches_full_compute(&session, "a long tail");
    assert_ne!(
        session.timeline().summaries().get(summary_of(1).0),
        before,
        "the summary finish did not follow the edit"
    );
}

#[test]
fn a_link_edit_then_a_duration_edit_then_undo_of_both() {
    let mut session = grouped();
    let before_all = session.timeline().summaries().clone();
    let pred = leaf(0, 24);
    let succ = leaf(3, 0);
    // The cone reads a solution equal to `compute()`; a link edit full-computes and rolls up in
    // full, so the duration edit after it starts from one.
    session.edit(add_link(pred, succ)).expect("a link");
    assert_matches_full_compute(&session, "the link edit");
    let after_link = session.timeline().summaries().clone();
    assert_ne!(after_link, before_all, "the link moves group 3");

    // Lengthening the head of group 3 moves its run, then the group's finish.
    let longer = duration(&session, succ) + 30 * DAY;
    set(&mut session, succ, longer);
    assert_matches_full_compute(&session, "the duration edit after the link");
    let after_duration = session.timeline().summaries().clone();
    assert_ne!(after_duration, after_link, "the duration moves group 3");

    // A second duration edit upstream of the link, which moves the linked task.
    let upstream = leaf(0, 20);
    let longer = duration(&session, upstream) + 5 * DAY;
    set(&mut session, upstream, longer);
    assert_matches_full_compute(&session, "a duration edit upstream of the link");
    let after_second = session.timeline().summaries().clone();

    assert!(session.undo().expect("undo"));
    assert_matches_full_compute(&session, "undo of the second duration edit");
    assert_eq!(session.timeline().summaries(), &after_duration);
    assert!(session.undo().expect("undo"));
    assert_matches_full_compute(&session, "undo of the first duration edit");
    assert_eq!(session.timeline().summaries(), &after_link);
    assert!(session.undo().expect("undo"));
    assert_matches_full_compute(&session, "undo of the link");
    assert_eq!(session.timeline().summaries(), &before_all);

    assert!(session.redo().expect("redo"));
    assert_matches_full_compute(&session, "redo of the link");
    assert_eq!(session.timeline().summaries(), &after_link);
    assert!(session.redo().expect("redo"));
    assert_matches_full_compute(&session, "redo of the first duration edit");
    assert_eq!(session.timeline().summaries(), &after_duration);
    assert!(session.redo().expect("redo"));
    assert_matches_full_compute(&session, "redo of the second duration edit");
    assert_eq!(session.timeline().summaries(), &after_second);
}

#[test]
fn a_duration_edit_after_each_kind_of_link_edit_matches_a_full_compute() {
    let mut session = grouped();
    let (pred, succ) = (leaf(0, 24), leaf(2, 0));
    session.edit(add_link(pred, succ)).expect("a link");
    let link = session
        .schedule()
        .live_dependencies()
        .find(|(_, dependency)| dependency.pred == pred && dependency.succ == succ)
        .map(|(id, _)| id)
        .expect("the added link is live");
    set(&mut session, succ, 3 * 480);
    assert_matches_full_compute(&session, "a duration edit on a linked task");

    session
        .edit(Edit::RemoveLink { dep: link })
        .expect("remove the link");
    assert_matches_full_compute(&session, "the link removed");
    let longer = duration(&session, succ) + 12 * DAY;
    set(&mut session, succ, longer);
    assert_matches_full_compute(&session, "a duration edit after the removal");
    set(&mut session, pred, 0);
    assert_matches_full_compute(&session, "a milestone");
    set(&mut session, pred, 0);
    assert_matches_full_compute(&session, "the same duration again");

    for _ in 0..4 {
        assert!(session.undo().expect("undo"));
        assert_matches_full_compute(&session, "undo");
    }
    while session.redo().expect("redo") {
        assert_matches_full_compute(&session, "redo");
    }
}

/// Edits to a leaf off the critical path: one so long that the project finish moves, a cut that
/// moves it back, a milestone, the same duration again and one minute more, then every undo.
#[test]
fn finish_moving_milestone_and_no_op_edits_match_a_full_compute() {
    let mut session = grouped();
    let task = leaf(2, 10);
    let original = duration(&session, task);
    let finish = session.solution().project_finish;
    for minutes in [
        original + 250 * DAY,
        0,
        original,
        original,
        original + 1,
        original,
    ] {
        set(&mut session, task, minutes);
        assert_matches_full_compute(&session, &format!("{minutes} minutes"));
        let moved = session.solution().project_finish != finish;
        assert_eq!(moved, minutes > 200 * DAY, "{minutes} minutes");
    }
    while session.undo().expect("undo") {
        assert_matches_full_compute(&session, "undo");
    }
    assert_eq!(duration(&session, task), original);
}

fn leaves(session: &Session) -> Vec<TaskId> {
    let outline = session.outline().expect("an outline");
    outline
        .preorder()
        .iter()
        .copied()
        .filter(|&task| !outline.is_summary(task))
        .collect()
}

/// Random duration edits, link edits, undo and redo over the demo outline, with a full compute
/// after every step. A link from an earlier leaf to a later one cannot close a loop, and a
/// refusal changes nothing, so the oracle holds either way.
#[test]
fn random_duration_link_undo_redo_sequences_match_a_full_compute() {
    let (mut session, _) = outline_session(800);
    let mut rng = Rng::new(SEED ^ 0x380D_C0DE);
    let candidates = leaves(&session);
    let (mut rolled_up, mut linked, mut undone, mut redone) = (0, 0, 0, 0);
    for step in 0..240 {
        let before = session.timeline().summaries().clone();
        let label = match step % 6 {
            0..=2 => {
                let task = candidates[rng.range(0, candidates.len() as i64 - 1) as usize];
                let minutes = match rng.range(0, 3) {
                    0 => 0,
                    1 => duration(&session, task) + rng.range(1, 600),
                    2 => rng.range(1, 3 * DAY),
                    _ => duration(&session, task) + rng.range(5, 40) * DAY,
                };
                set(&mut session, task, minutes);
                format!("step {step}: duration {minutes} on {task}")
            }
            3 => {
                let a = candidates[rng.range(0, candidates.len() as i64 - 1) as usize];
                let b = candidates[rng.range(0, candidates.len() as i64 - 1) as usize];
                let (pred, succ) = if session.solution().get(a).es <= session.solution().get(b).es {
                    (a, b)
                } else {
                    (b, a)
                };
                if session.edit(add_link(pred, succ)).is_ok() {
                    linked += 1;
                }
                format!("step {step}: link {pred} to {succ}")
            }
            4 => {
                undone += usize::from(session.undo().expect("undo"));
                format!("step {step}: undo")
            }
            _ => {
                redone += usize::from(session.redo().expect("redo"));
                format!("step {step}: redo")
            }
        };
        assert_matches_full_compute(&session, &label);
        if step % 6 < 3 && session.timeline().summaries() != &before {
            rolled_up += 1;
        }
    }
    assert!(linked >= 5, "links applied: {linked}");
    assert!(
        undone >= 5 && redone >= 5,
        "undone {undone}, redone {redone}"
    );
    assert!(
        rolled_up >= 5,
        "duration edits that moved a summary: {rolled_up}"
    );
}

#[test]
fn a_refused_duration_changes_nothing() {
    let mut session = grouped();
    let task = leaf(0, 3);
    let (tasks, solution, summaries) = (
        session.schedule().tasks().to_vec(),
        session.solution().clone(),
        session.timeline().summaries().clone(),
    );
    session
        .edit(Edit::SetDuration { task, minutes: -1 })
        .expect_err("a negative duration is refused");
    session
        .edit(Edit::SetDuration {
            task: TaskId(9_999),
            minutes: 480,
        })
        .expect_err("an unknown task is refused");
    assert_eq!(session.schedule().tasks(), tasks);
    assert_eq!(session.solution(), &solution);
    assert_eq!(session.timeline().summaries(), &summaries);
    assert_matches_full_compute(&session, "the refusals");
}

/// How the engine solves `task` at `minutes` from the session's present state, run on clones: the
/// app does not see which path the engine took, so the three round-trip tests below check that
/// their fixture takes the path each is named for.
fn solved_by(session: &Session, task: TaskId, minutes: Minutes) -> DurationCone {
    let mut schedule = session.schedule().clone();
    let mut solution = session.solution().clone();
    schedule.set_duration(task, minutes).expect("the duration");
    schedule
        .recompute_duration_cone_moved(&mut solution, task, &mut Vec::new())
        .expect("the cone solves")
}

/// A duration edit, its undo and its redo, each held to a full compute. The edit must move a
/// summary, and the undo and redo must land on the spans before and after it.
fn edit_undo_redo(session: &mut Session, task: TaskId, minutes: Minutes, label: &str) {
    let original = duration(session, task);
    let before = session.timeline().summaries().clone();
    set(session, task, minutes);
    assert_matches_full_compute(session, &format!("{label}: the edit"));
    let after = session.timeline().summaries().clone();
    assert_ne!(after, before, "{label}: the edit moves a summary");

    assert!(session.undo().expect("undo"), "{label}: an entry to undo");
    assert_matches_full_compute(session, &format!("{label}: the undo"));
    assert_eq!(session.timeline().summaries(), &before, "{label}: undo");
    assert_eq!(
        duration(session, task),
        original,
        "{label}: undone duration"
    );

    assert!(session.redo().expect("redo"), "{label}: an entry to redo");
    assert_matches_full_compute(session, &format!("{label}: the redo"));
    assert_eq!(session.timeline().summaries(), &after, "{label}: redo");
    assert_eq!(duration(session, task), minutes, "{label}: redone duration");
}

#[test]
fn undo_and_redo_restore_the_summaries_of_a_cone_edit() {
    let mut session = grouped();
    let tail = leaf(1, 24);
    let minutes = duration(&session, tail) + 40 * DAY;
    assert_eq!(solved_by(&session, tail, minutes), DurationCone::Cone);
    edit_undo_redo(&mut session, tail, minutes, "a cone edit");
}

#[test]
fn undo_and_redo_restore_the_summaries_of_a_finish_moving_edit() {
    let mut session = grouped();
    let task = leaf(2, 10);
    let minutes = duration(&session, task) + 250 * DAY;
    assert_eq!(
        solved_by(&session, task, minutes),
        DurationCone::Fallback(FallbackReason::FinishMove)
    );
    edit_undo_redo(&mut session, task, minutes, "a finish-moving edit");
}

/// 104 tasks give the cone a limit of 11. A hub in group 1 with 14 successors in group 2 moves
/// more than that when it grows, and the 200-day leaf keeps the project finish where it is, so
/// the limit, not a finish move, sends the edit to the full compute.
#[test]
fn undo_and_redo_restore_the_summaries_of_an_edit_wider_than_the_cone_limit() {
    let mut session = grouped();
    let hub = leaf(1, 0);
    for index in 0..14 {
        session
            .edit(add_link(hub, leaf(2, index)))
            .expect("a hub link");
    }
    assert_matches_full_compute(&session, "the hub links");
    let minutes = duration(&session, hub) + 30 * DAY;
    assert_eq!(
        solved_by(&session, hub, minutes),
        DurationCone::Fallback(FallbackReason::ForwardLimit)
    );
    edit_undo_redo(&mut session, hub, minutes, "a wide edit");
}
