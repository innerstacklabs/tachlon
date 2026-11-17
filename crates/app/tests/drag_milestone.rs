//! The bar-drag path of the app, without a canvas: a milestone dragged to the
//! end of a working period, and pulled back to its press start.

use crate::common;

use common::clock;

use app::{DragInfo, Session};
use engine::{Calendar, DAY, Dependency, LinkType, Schedule, Task};
use timeline::Timeline;

/// Minutes into the day of 12:00 and 13:00 on the 08:00-12:00 / 13:00-17:00
/// calendar of the milestone tests.
const NOON: i64 = 12 * 60;
const ONE_PM: i64 = 13 * 60;

/// A milestone on the 08:00-12:00 / 13:00-17:00 calendar, at 13:00 on day 1
/// (a start-no-earlier-than), or with a FS predecessor milestone at 13:00 and
/// the milestone itself at 14:00, so the predecessor holds the ask for 12:00
/// at 13:00, a start that is not the press start. Drags it to 12:00, the end
/// of the morning period, and returns the session, the milestone and what
/// the drag gave.
fn drag_milestone_to_noon(with_predecessor: bool) -> (Session, u32, DragInfo) {
    let pressed_at = DAY + ONE_PM + if with_predecessor { 60 } else { 0 };
    drag_milestone_to_noon_from(with_predecessor, pressed_at)
}

/// [`drag_milestone_to_noon`] with the milestone's start at the press given:
/// the predecessor, if any, is a milestone at 13:00.
fn drag_milestone_to_noon_from(
    with_predecessor: bool,
    pressed_at: i64,
) -> (Session, u32, DragInfo) {
    let (mut s, m, (x, y)) = milestone_session(with_predecessor, pressed_at);
    s.pointer_down(x, y, false);
    s.pointer_move(x - (pressed_at - (DAY + NOON)) as f64, y);
    s.pointer_up();
    let info = s
        .drag_info()
        .copied()
        .expect("the release recorded the drag");
    (s, m, info)
}

/// The session of [`drag_milestone_to_noon_from`] before the press: the
/// session, the milestone and the point on its square to press on.
#[expect(
    clippy::many_single_char_names,
    reason = "m and p are the milestone and its predecessor"
)]
fn milestone_session(with_predecessor: bool, pressed_at: i64) -> (Session, u32, (f64, f64)) {
    let mut schedule = Schedule::new();
    let day = vec![(8 * 60, NOON as u16), (ONE_PM as u16, 17 * 60)];
    let cal = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("valid calendar");
    let cal = schedule.add_calendar(cal);
    let at_one_pm = DAY + ONE_PM;
    let milestone = |snet| Task {
        duration: 0,
        calendar: cal,
        snet,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    let m = schedule
        .add_task(milestone(Some(pressed_at)))
        .expect("valid task");
    if with_predecessor {
        let p = schedule
            .add_task(milestone(Some(at_one_pm)))
            .expect("valid task");
        schedule
            .add_dependency(Dependency {
                pred: p,
                succ: m,
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("valid dependency");
    }
    let solution = schedule.compute().expect("acyclic");
    assert_eq!(solution.dates[m.0 as usize].es, pressed_at);
    let order = timeline::order_by_start(&solution.dates);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    // One pixel per minute, the press start at x = 500, so the drag to 12:00
    // is 60 px to the left, or 120 px with the predecessor.
    let v = &mut timeline.viewport;
    v.px_per_minute = 1.0;
    v.t0 = pressed_at as f64 - 500.0;
    let row = timeline
        .order()
        .iter()
        .position(|&t| t == m.0)
        .expect("the milestone has a row");
    let x = timeline.viewport.x_of(pressed_at as f64);
    let y = timeline.viewport.row_top(row) + timeline.viewport.row_height / 2.0;
    let s = Session::new(schedule, solution, timeline, Vec::new(), clock);
    // The press must land on the milestone's square, or the test checks
    // nothing.
    assert_eq!(s.timeline().hit_test(&s.solution().dates, x, y), Some(m.0));
    (s, m.0, (x, y))
}

#[test]
fn a_milestone_held_past_a_period_end_is_held() {
    // 12:00 is an admissible start for a milestone (the morning period's
    // end), and a finish-to-start predecessor at 13:00 holds it there.
    let (s, m, info) = drag_milestone_to_noon(true);
    let noon = DAY + NOON;
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(noon));
    assert_eq!(info.wanted, noon);
    assert_eq!(info.es, DAY + ONE_PM);
    assert!(info.held, "the predecessor holds it past the ask");
}

#[test]
fn a_milestone_held_at_its_press_start_reads_as_held_when_pulled_to_noon() {
    // The predecessor holds the milestone at 13:00, its press start, so the
    // ask for 12:00 changes nothing, and the task is still held later than
    // the pointer asked.
    let (s, m, info) = drag_milestone_to_noon_from(true, DAY + ONE_PM);
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(DAY + ONE_PM));
    assert_eq!(s.revision(), 0);
    assert_eq!(info.wanted, DAY + NOON);
    assert_eq!(info.es, DAY + ONE_PM);
    assert!(info.held, "the predecessor holds it at the press start");
}

/// The milestone of the test above, pressed at 13:00 with its own snet and the
/// predecessor both there, and pulled to noon on the first frame: the ask
/// changes nothing and the milestone is held. Returns the session, the
/// milestone and the press point.
fn milestone_pulled_to_noon() -> (Session, u32, (f64, f64)) {
    let at_one_pm = DAY + ONE_PM;
    let (mut s, m, (x, y)) = milestone_session(true, at_one_pm);
    s.pointer_down(x, y, false);
    assert!(s.drag_info().is_none(), "the press cleared the diagnostic");
    s.pointer_move(x - (at_one_pm - (DAY + NOON)) as f64, y);
    s.apply_drag();
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(at_one_pm));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    let info = s.drag_info().expect("the frame recorded the drag");
    assert_eq!(info.wanted, DAY + NOON);
    assert!(info.held, "the predecessor holds it past the ask");
    assert!(s.held());
    (s, m, (x, y))
}

/// Back at the press point, the ask equals the early start: the milestone is
/// no longer held, and the diagnostic says so, though the schedule already
/// has this ask and nothing is recomputed. The frame changes no snet,
/// revision or stamp.
#[test]
fn a_milestone_pulled_back_to_its_press_start_stops_showing_held() {
    let (mut s, m, (x, y)) = milestone_pulled_to_noon();
    s.pointer_move(x, y);
    s.apply_drag();
    let info = s.drag_info().expect("the frame recorded the drag");
    assert_eq!(info.wanted, DAY + ONE_PM);
    assert_eq!(info.es, DAY + ONE_PM);
    assert!(!info.held, "the ask equals the early start");
    assert!(!s.held());
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(DAY + ONE_PM));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
    s.pointer_up();
    assert!(!s.drag_info().expect("still recorded").held);
    assert!(!s.held());
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(DAY + ONE_PM));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
}

/// The same, with no frame at the press point: the release applies the ask.
#[test]
fn a_milestone_pulled_back_and_released_between_frames_stops_showing_held() {
    let (mut s, m, (x, y)) = milestone_pulled_to_noon();
    s.pointer_move(x, y);
    s.pointer_up();
    let info = s.drag_info().expect("the release recorded the drag");
    assert_eq!(info.wanted, DAY + ONE_PM);
    assert!(!info.held, "the ask equals the early start");
    assert!(!s.held());
    assert_eq!(s.schedule().tasks()[m as usize].snet, Some(DAY + ONE_PM));
    assert_eq!(s.revision(), 0);
    assert_eq!(s.stamp(), 0);
}

#[test]
fn an_unconstrained_milestone_lands_on_a_period_end() {
    let (s, m, info) = drag_milestone_to_noon(false);
    let noon = DAY + NOON;
    assert_eq!(info.wanted, noon);
    assert_eq!(info.es, noon);
    assert_eq!(s.solution().dates[m as usize].es, noon);
    assert!(!info.held);
}
