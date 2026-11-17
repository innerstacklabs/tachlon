//! A dragged bar lands on a working-day start of its calendar, not at the
//! minute the pointer let go, so its successors keep whole working days.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use common::clock;

use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task};
use timeline::Timeline;

use app::Session;

/// 08:00 on a day, in minutes.
const OPEN: i64 = 8 * 60;

/// Two one-day (9 h) tasks on a Monday-to-Friday 08:00-17:00 calendar, the second
/// after the first. One pixel is ten minutes. Returns the session, the two
/// tasks, the first one's start and the canvas point to press on its bar.
fn chain() -> (Session, u32, u32, i64, (f64, f64)) {
    chain_with(&[(8 * 60, 17 * 60)])
}

/// [`chain`], with `day` as the working periods of each weekday.
fn chain_with(day: &[(u16, u16)]) -> (Session, u32, u32, i64, (f64, f64)) {
    let mut schedule = Schedule::new();
    let week = std::array::from_fn(|d| if d < 5 { day.to_vec() } else { Vec::new() });
    let cal = schedule.add_calendar(Calendar::new(week, &[]).expect("valid calendar"));
    let work = |snet| Task {
        duration: 9 * 60,
        calendar: cal,
        snet,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    let first = schedule.add_task(work(None)).expect("valid task");
    let second = schedule.add_task(work(None)).expect("valid task");
    schedule
        .add_dependency(Dependency {
            pred: first,
            succ: second,
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("valid dependency");
    let solution = schedule.compute().expect("acyclic");
    let es = solution.dates[first.0 as usize].es;
    assert_eq!(es, OPEN, "the first task opens on Monday at 08:00");
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    let v = &mut timeline.viewport;
    v.px_per_minute = 0.1;
    v.t0 = es as f64 - 500.0;
    let row = timeline.order().iter().position(|&t| t == first.0).unwrap();
    let x = timeline.viewport.x_of(es as f64) + 2.0;
    let y = timeline.viewport.row_top(row) + timeline.viewport.row_height / 2.0;
    let session = Session::new(schedule, solution, timeline, Vec::new(), clock);
    assert_eq!(
        session.timeline().hit_test(&session.solution().dates, x, y),
        Some(first.0)
    );
    (session, first.0, second.0, es, (x, y))
}

/// Dragging 186 px asks for Tuesday 15:00, a working minute in the middle of
/// the day. The task must start at 08:00 that day, and the successor keeps whole days too.
#[test]
fn a_release_mid_day_lands_on_the_day_start() {
    let (mut s, first, second, es, (x, y)) = chain();
    s.pointer_down(x, y, false);
    s.pointer_move(x + 186.0, y);
    s.apply_drag();
    let live = s.solution().dates[first as usize].es;
    assert_eq!(live, DAY + OPEN, "the live preview shows the day start");
    s.pointer_up();
    let dates = &s.solution().dates;
    assert_eq!(dates[first as usize].es, DAY + OPEN);
    assert_eq!(dates[first as usize].es % DAY, OPEN);
    assert_eq!(dates[second as usize].es % DAY, OPEN, "the successor too");
    assert!(dates[first as usize].es > es);
}

/// A day with a lunch gap has one start. Tuesday 14:00 is working time after
/// the gap, and the task still opens the day at 08:00.
#[test]
fn a_release_after_a_midday_gap_lands_on_the_day_start() {
    let (mut s, first, _, _, (x, y)) = chain_with(&[(8 * 60, 12 * 60), (13 * 60, 17 * 60)]);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 180.0, y);
    s.apply_drag();
    assert_eq!(s.solution().dates[first as usize].es, DAY + OPEN);
    s.pointer_up();
    assert_eq!(s.solution().dates[first as usize].es, DAY + OPEN);
}
