//! The move report of a bar drag (gs#424, story S2 of #309): a drag runs one edit per frame, so
//! the report is made at the release from the journal of the first old dates, not the latest.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use common::{MINUTES_PER_PX, clock, setup_with};
use engine::{LinkType, Schedule, Task, TaskId};
use std::collections::BTreeSet;
use timeline::Timeline;

fn early(s: &app::Session) -> Vec<(i64, i64)> {
    s.solution().dates.iter().map(|d| (d.es, d.ef)).collect()
}

fn assert_equals_compute(s: &app::Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

#[test]
fn a_drag_over_many_frames_reports_the_press_time_positions() {
    let (mut s, task, _, (x, y)) = setup_with(true);
    let press = early(&s);
    s.pointer_down(x, y, false);

    s.pointer_move(x + 60.0, y);
    s.apply_drag();
    assert_equals_compute(&s);
    let first = early(&s);
    let differs = |a: &[(i64, i64)], b: &[(i64, i64)]| -> BTreeSet<usize> {
        (0..a.len())
            .filter(|&i| i != task as usize && a[i] != b[i])
            .collect()
    };
    let moved_in_first = differs(&press, &first);
    assert!(
        !moved_in_first.is_empty(),
        "the first frame moved no other task"
    );

    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    assert_equals_compute(&s);
    let last = early(&s);
    // The check needs a task that moved in both frames: its first-frame dates are not its
    // press-time ones, so a journal that kept the latest old dates would show the wrong ones.
    assert!(
        moved_in_first.iter().any(|&i| last[i] != first[i]),
        "no task moved in both frames"
    );
    assert!(s.move_report().is_none(), "frames make no report");

    s.pointer_up();
    assert_equals_compute(&s);
    let r = s.move_report().expect("a drag that moved others reports");
    let expected = differs(&press, &last);
    let reported: BTreeSet<usize> = r.was.iter().map(|m| m.task.0 as usize).collect();
    assert_eq!(reported, expected);
    for m in &r.was {
        assert_eq!(
            (m.was_es, m.was_ef),
            press[m.task.0 as usize],
            "task {}",
            m.task.0
        );
    }
    assert_eq!(r.others as usize, expected.len());
    // The dragged task is not an "other", and its old dates are the press-time ones, not the
    // ones it had before the last frame.
    assert!(r.was.iter().all(|m| m.task.0 != task));
    assert_ne!(first[task as usize], press[task as usize]);
    assert_ne!(last[task as usize], first[task as usize]);
    assert_eq!(
        r.edited,
        Some(engine::Moved {
            task: engine::TaskId(task),
            was_es: press[task as usize].0,
            was_ef: press[task as usize].1,
        })
    );
    assert!(!r.slide);
    assert!(
        r.text.starts_with(&format!("{} task", r.others)),
        "{}",
        r.text
    );
}

#[test]
fn a_drag_that_returns_to_its_start_makes_no_report() {
    let (mut s, _, _, (x, y)) = setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    s.pointer_move(x, y);
    s.apply_drag();
    s.pointer_up();
    assert!(s.move_report().is_none());
    assert_equals_compute(&s);
}

/// The start-no-earlier-than a drag of `x` px sets on the task at the press point, found on a
/// session of its own so the caller can replay it as an edit.
fn dragged_snet(dx: f64) -> Option<engine::Instant> {
    let (mut s, task, _, (x, y)) = setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + dx, y);
    s.apply_drag();
    s.schedule().tasks()[task as usize].snet
}

#[test]
fn a_cancelled_drag_makes_no_report_and_the_next_edit_of_the_task_does() {
    let snet = dragged_snet(200.0);
    let (mut s, task, _, (x, y)) = setup_with(true);
    let press = early(&s);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    assert_ne!(early(&s), press, "the drag moved nothing");
    s.pointer_cancel();
    assert!(s.move_report().is_none());
    assert_eq!(early(&s), press, "a cancel restores the press-time dates");
    assert_equals_compute(&s);

    // The journal of the cancelled press is gone: the same task's next edit reports.
    s.edit(app::edit::Edit::Move {
        task: engine::TaskId(task),
        snet,
    })
    .expect("applies");
    let r = s.move_report().expect("the edit after a cancel reports");
    assert!(r.slide && r.others > 0, "{r:?}");
    assert_equals_compute(&s);
}

/// The release of a drag is lost; `second_press` is the next press. It must end the first
/// drag's journal, or the later edit of the same task is taken for a frame of the first drag.
fn a_lost_release_then(second_press: impl FnOnce(&mut app::Session, f64, f64)) {
    let snet = dragged_snet(200.0);
    let (mut s, task, _, (x, y)) = setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 60.0, y);
    s.apply_drag();
    // A press is ignored while another is down: the page cancels the press whose release was lost.
    s.pointer_cancel();
    second_press(&mut s, x, y);
    s.edit(app::edit::Edit::Move {
        task: engine::TaskId(task),
        snet,
    })
    .expect("applies");
    let r = s.move_report().expect("the edit reports");
    assert!(r.slide && r.others > 0, "{r:?}");
}

#[test]
fn a_lost_release_then_an_alt_press_does_not_swallow_a_later_edit() {
    a_lost_release_then(|s, x, y| s.pointer_down(x, y, true));
}

#[test]
fn a_lost_release_then_a_press_on_empty_space_does_not_swallow_a_later_edit() {
    a_lost_release_then(|s, x, y| s.pointer_down(x, y + 5000.0, false));
}

#[test]
fn an_edit_during_a_press_that_has_not_started_a_drag_reports() {
    let snet = dragged_snet(200.0);
    let (mut s, task, _, (x, y)) = setup_with(true);
    s.pointer_down(x, y, false);
    // The pointer has not moved past the drag threshold: this edit is not a drag frame.
    s.edit(app::edit::Edit::Move {
        task: engine::TaskId(task),
        snet,
    })
    .expect("applies");
    let r = s.move_report().expect("the edit reports");
    assert!(r.slide && r.others > 0, "{r:?}");
    s.pointer_up();
    assert_equals_compute(&s);
}

/// Two chains on a Monday-to-Friday calendar of eight-hour days: A (10 days) then B (10 days),
/// and C (15 days) then D (10 days). C and D are critical; A and B have 5 days of float. The
/// viewport shows A's bar at its left, `MINUTES_PER_PX` minutes to the pixel; returns the session
/// and the point on A's bar.
fn two_chains() -> (app::Session, (f64, f64)) {
    two_chains_pressing(0)
}

/// [`two_chains`] with the press on the bar of `press_task` instead of A's.
fn two_chains_pressing(press_task: u32) -> (app::Session, (f64, f64)) {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    for days in [10, 10, 15, 10] {
        schedule
            .add_task(Task {
                duration: days * 480,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
    }
    for (pred, succ) in [(0, 1), (2, 3)] {
        schedule
            .add_link(engine::Dependency {
                pred: TaskId(pred),
                succ: TaskId(succ),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("a link");
    }
    let solution = schedule.compute().expect("acyclic");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1200.0, 600.0, 1.0);
    let es = solution.dates[press_task as usize].es;
    let row = timeline
        .row_of_task(press_task)
        .expect("the task has a row");
    let v = &mut timeline.viewport;
    v.px_per_minute = 1.0 / MINUTES_PER_PX;
    v.t0 = es as f64 - 50.0 * MINUTES_PER_PX;
    let point = (v.x_of(es as f64) + 2.0, v.row_top(row) + v.row_height / 2.0);
    let s = app::Session::new(schedule, solution, timeline, Vec::new(), clock);
    assert_eq!(
        s.timeline().hit_test(&s.solution().dates, point.0, point.1),
        Some(press_task),
        "the press must land on the task"
    );
    (s, point)
}

fn critical(s: &app::Session) -> Vec<bool> {
    let fresh = s.schedule().clone().compute().expect("acyclic");
    fresh
        .dates
        .iter()
        .map(engine::TaskDates::is_critical)
        .collect()
}

#[test]
fn a_drag_release_names_the_critical_path_change() {
    let (mut s, (x, y)) = two_chains();
    let before = critical(&s);
    assert_eq!(before, [false, false, true, true]);
    s.pointer_down(x, y, false);
    // Many frames, each a full edit: the before-state is the press-time one, not a frame's.
    for step in 1..=20 {
        s.pointer_move(x + 20.0 * f64::from(step), y);
        s.apply_drag();
    }
    s.pointer_up();
    let after = critical(&s);
    assert_eq!(after, [true, true, false, false], "A and B took the path");
    let count = |from: bool| {
        before
            .iter()
            .zip(&after)
            .filter(|&(&b, &a)| b == from && a != from)
            .count() as u32
    };
    let (joined, left) = (count(false), count(true));
    assert_eq!((joined, left), (2, 2));
    let r = s.move_report().expect("B moved");
    assert_eq!((r.joined, r.left), (joined, left), "{}", r.text);
    assert!(
        r.text.ends_with(" · critical 2 joined, 2 left"),
        "{}",
        r.text
    );
    assert_equals_compute(&s);
}

/// gs#703: a release that moves no other task makes no report, so no note and no Show moved
/// button. This is deliberate: the dragged task is not an "other". B is the tail of chain A to B,
/// with 5 days of float and no successor, so a later drag of less than that moves nothing else.
#[test]
fn a_tail_task_drag_within_its_float_makes_no_report() {
    let (mut s, (x, y)) = two_chains_pressing(1);
    let press = early(&s);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 12.0, y);
    s.apply_drag();
    s.pointer_up();
    let after = early(&s);
    assert!(after[1].0 > press[1].0, "the drag moved B later");
    for other in [0, 2, 3] {
        assert_eq!(after[other], press[other], "task {other} must not move");
    }
    assert_equals_compute(&s);
    assert!(s.move_report().is_none(), "{:?}", s.move_report());
    let json = s.status_json(&app::RenderFacts::default());
    assert!(json.contains(r#""moved":null"#), "{json}");
}
