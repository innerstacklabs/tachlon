//! `status.offscreen` and `Session::go_to_offscreen` (gs#309e1): when no row the window shows has
//! work in the window's time span, the status names the nearest finish before and the nearest
//! start after, among those rows; when one has, it is `null`.
//!
//! The fixture is three tasks on a 24/7 calendar, one row each, starting in weeks 0, 30 and 31
//! from the demo origin (Monday 2026-01-05), so the dates below are written from the calendar:
//! week 0 is 5 Jan, week 30 starts on 3 Aug, week 31 is 10 Aug.

use crate::common;

use app::{RenderFacts, Session};
use common::clock;
use engine::{Calendar, DAY, Schedule, Task, WEEK};
use timeline::Timeline;

fn session() -> Session {
    let day = vec![(0, 1_440)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    for snet in [0, 30 * WEEK, 31 * WEEK] {
        schedule
            .add_task(Task {
                duration: DAY,
                calendar,
                snet: Some(snet),
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    let solution = schedule.clone().compute().expect("schedules");
    let meta = (0..3).map(|_| app::meta::TaskMeta::default()).collect();
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
}

/// The status JSON's `offscreen` member.
fn field(session: &Session) -> serde_json::Value {
    let json = session.status_json(&RenderFacts::default());
    let status: serde_json::Value = serde_json::from_str(&json).expect("status is JSON");
    status
        .get("offscreen")
        .expect("status has an offscreen member")
        .clone()
}

/// Shows 100,000 minutes (about ten weeks) from the start of week `week`.
fn show_from(session: &mut Session, week: i64) {
    common::zoom_to(session, 100.0, (week * WEEK) as f64);
}

#[test]
fn a_window_over_an_empty_stretch_names_the_work_before_and_after() {
    let mut s = session();
    show_from(&mut s, 12);
    let o = field(&s);
    assert_eq!(o["b"], "5 Jan", "the nearest finish before the window");
    assert_eq!(o["a"], "3 Aug", "the nearest start after it");
}

#[test]
fn a_window_with_work_reports_none() {
    let mut s = session();
    show_from(&mut s, 0);
    assert!(field(&s).is_null(), "week 0 has a bar");
    show_from(&mut s, 28);
    assert!(field(&s).is_null(), "weeks 30 and 31 are in the window");
}

#[test]
fn only_one_side_is_named_when_the_other_has_no_work() {
    let mut s = session();
    show_from(&mut s, 45);
    let o = field(&s);
    assert_eq!(o["b"], "10 Aug", "week 31 is 10 Aug");
    assert!(o["a"].is_null());
}

#[test]
fn the_note_costs_at_most_sixty_bytes_over_null() {
    let mut s = session();
    show_from(&mut s, 0);
    assert!(field(&s).is_null());
    let idle = s.status_json(&RenderFacts::default()).len();
    show_from(&mut s, 12);
    let empty = s.status_json(&RenderFacts::default()).len();
    assert!(empty - idle <= 60, "{} bytes", empty - idle);
}

#[test]
fn a_jump_brings_the_named_work_into_view_and_selects_nothing() {
    let mut s = session();
    show_from(&mut s, 12);
    let rows = s.timeline().viewport.visible_rows();
    assert!(s.go_to_offscreen(true));
    assert!(field(&s).is_null(), "the later work is in the window");
    assert_eq!(s.timeline().viewport.visible_rows(), rows, "rows stay");
    assert!(s.selection().is_none());

    show_from(&mut s, 12);
    assert!(s.go_to_offscreen(false));
    assert!(field(&s).is_null(), "the earlier work is in the window");

    assert!(
        !s.go_to_offscreen(true),
        "nothing to jump to with work in view"
    );
}
