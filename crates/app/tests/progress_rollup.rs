//! The % complete of a summary row (gs#703): the share of the work under it that is done, weighted
//! by duration. The calendar is Monday to Friday, 08:00-12:00 and 13:00-17:00, so a day is 480
//! minutes and a 2-day task is 960. Instant 0 is Monday 08:00 of the demo origin.

use crate::common;

use app::{Session, dates::Origin, meta::TaskMeta, outline::Outline};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Minutes, Schedule, Task, TaskId};
use timeline::Timeline;

const DAY: Minutes = 480;

/// A leaf that has not begun.
fn unstarted(days: i64) -> (Minutes, Option<Instant>, Option<Instant>) {
    (days * DAY, None, None)
}

/// A leaf finished on its last day.
fn finished(days: i64) -> (Minutes, Option<Instant>, Option<Instant>) {
    (days * DAY, Some(0), Some(days * 1440))
}

type Leaf = (Minutes, Option<Instant>, Option<Instant>);

/// The schedule, meta and outline of a summary (task 0) over the given leaves (tasks 1..).
fn parts(leaves: &[Leaf]) -> (Schedule, Vec<TaskMeta>, Outline) {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let mut meta = vec![TaskMeta::default()];
    let add = |schedule: &mut Schedule, (duration, start, finish)| {
        schedule
            .add_task(Task {
                duration,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: start,
                actual_finish: finish,
                remaining: None,
            })
            .expect("the calendar belongs to the schedule");
    };
    add(&mut schedule, unstarted(1));
    for &leaf in leaves {
        add(&mut schedule, leaf);
        meta.push(TaskMeta {
            parent: Some(TaskId(0)),
            ..TaskMeta::default()
        });
    }
    let wbs: Vec<TaskId> = (0..meta.len() as u32).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    (schedule, meta, outline)
}

/// A session whose task 0 is a summary over the given leaves (tasks 1..).
fn session(leaves: &[Leaf]) -> Session {
    let (mut schedule, meta, outline) = parts(leaves);
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new((0..meta.len() as u32).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline matches the schedule")
}

#[test]
fn a_summary_reads_the_duration_weighted_percent_of_its_leaves() {
    let two_days = session(&[finished(2), unstarted(2)]);
    assert_eq!(two_days.task_percent(0), "50%");
    assert_eq!(two_days.task_percent(1), "100%", "a leaf is unchanged");
    assert_eq!(two_days.task_percent(2), "");

    // Weighted by duration, not by count: a finished day against three days not begun.
    let weighted = session(&[finished(1), unstarted(3)]);
    assert_eq!(weighted.task_percent(0), "25%");

    // Nothing under it has started: empty, as for a leaf.
    let unbegun = session(&[unstarted(2), unstarted(2)]);
    assert_eq!(unbegun.task_percent(0), "");
}

#[test]
fn a_summary_counts_the_started_part_of_a_leaf_by_its_remaining() {
    // One 2-day leaf begun with one day left, one 2-day leaf not begun: 1 of 4 days done.
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for (duration, start, remaining) in [
        (DAY, None, None),
        (2 * DAY, Some(0), Some(DAY)),
        (2 * DAY, None, None),
    ] {
        schedule
            .add_task(Task {
                duration,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: start,
                actual_finish: None,
                remaining,
            })
            .expect("the calendar belongs to the schedule");
    }
    let meta = vec![
        TaskMeta::default(),
        TaskMeta {
            parent: Some(TaskId(0)),
            ..TaskMeta::default()
        },
        TaskMeta {
            parent: Some(TaskId(0)),
            ..TaskMeta::default()
        },
    ];
    let wbs = vec![TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let s = Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("the outline matches the schedule");
    assert_eq!(s.task_percent(0), "25%");
}

#[test]
fn the_sums_are_kept_per_stamp_and_follow_an_edit() {
    let mut s = session(&[finished(2), unstarted(2)]);
    assert_eq!(s.task_percent(0), "50%");
    assert_eq!(s.task_percent(0), "50%");
    assert_eq!(
        s.progress_rollup_runs(),
        1,
        "a second read at rest sums nothing"
    );

    // Finishing the other leaf moves the stamp; the summary reads 100% from a fresh sum.
    s.set_progress_text(TaskId(2), "2026-01-05", "2026-01-06", "")
        .expect("a leaf takes progress");
    assert_eq!(s.task_percent(0), "100%");
    assert_eq!(s.progress_rollup_runs(), 2);

    // Undo puts the schedule back, and the summary with it.
    s.undo().expect("undo");
    assert_eq!(s.task_percent(0), "50%");
}

#[test]
fn a_summary_of_milestones_reads_empty() {
    // Nothing to weigh: a milestone has no duration, begun or not.
    let milestones = session(&[unstarted(0), unstarted(0)]);
    assert_eq!(milestones.task_percent(0), "");
    let reached = session(&[finished(0), finished(0)]);
    assert_eq!(reached.task_percent(0), "");

    // A milestone beside a finished task weighs nothing: the summary reads the task alone.
    let mixed = session(&[unstarted(0), finished(2)]);
    assert_eq!(mixed.task_percent(0), "100%");
}

#[test]
fn a_second_file_of_the_same_size_does_not_show_the_first_files_percents() {
    let first = session(&[finished(2), unstarted(2)]);
    assert_eq!(
        first.task_percent(0),
        "50%",
        "the first file's sums are kept"
    );

    // Open another file of the same task count, at rest (a reopen moves the stamp past the
    // first's, but the memo must not depend on that).
    let (mut schedule, meta, outline) = parts(&[unstarted(2), unstarted(2)]);
    let solution = schedule.compute().expect("computes");
    let second = first
        .reopen(
            schedule,
            solution,
            (0..meta.len() as u32).collect(),
            meta,
            Some(outline),
            None,
            Origin::DEMO,
        )
        .expect("the outline matches its schedule");
    assert_eq!(second.task_percent(0), "");
    assert_eq!(
        second.progress_rollup_runs(),
        1,
        "the second file sums for itself"
    );
    assert_eq!(first.task_percent(0), "50%", "the first file is unchanged");
}
