//! Fit and find fly (gs#309): the viewport eases to its target over 300 ms, and a wheel, a drag or a
//! key ends the flight where the person put the view. Reduced motion jumps. Time is a fake clock
//! the test sets, so a frame is exact.

use app::{Session, fly::FLY_MS};
use std::cell::Cell;
use timeline::Timeline;

thread_local! {
    /// The fake clock. Each test runs on its own thread, so each has its own time.
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.with(Cell::get)
}

fn at(ms: f64) {
    NOW.with(|now| now.set(ms));
}

/// The demo session on the fake clock, zoomed in so Fit has somewhere to go.
fn zoomed_in() -> Session {
    at(1_000.0);
    let (schedule, solution, order, meta) = app::demo(2_000);
    let mut session = Session::new(
        schedule,
        solution,
        Timeline::new(order, 1_000.0, 400.0, 1.0),
        meta,
        clock,
    );
    session.resize(1_000.0, 400.0, 1.0);
    for _ in 0..2 {
        session.wheel(500.0, 0.0, 0.0, -350.0, true);
    }
    session
}

fn view(session: &Session) -> (f64, f64) {
    let v = &session.timeline().viewport;
    (v.t0, v.px_per_minute)
}

/// The view Fit lands on, and the view before it, from a session that jumped.
fn start_and_target() -> ((f64, f64), (f64, f64)) {
    let mut session = zoomed_in();
    let start = view(&session);
    assert!(session.fit_project());
    session.land_fly();
    (start, view(&session))
}

#[test]
fn fit_flies_from_the_start_through_the_middle_to_the_target() {
    let (start, target) = start_and_target();
    assert_ne!(start, target);

    let mut session = zoomed_in();
    assert!(session.fit_project());
    session.advance_fly();
    assert_eq!(view(&session), start, "t = 0 is where the view was");

    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    let (t0, scale) = view(&session);
    let between = |x: f64, a: f64, b: f64| x > a.min(b) && x < a.max(b);
    assert!(between(t0, start.0, target.0), "t0 {t0} is between");
    assert!(
        between(scale, start.1, target.1),
        "scale {scale} is between"
    );
    assert!(session.flying());

    at(1_000.0 + FLY_MS);
    session.advance_fly();
    assert_eq!(view(&session), target, "the end is the target");
    assert!(!session.flying());
}

#[test]
fn a_wheel_mid_fly_leaves_the_view_where_the_wheel_put_it() {
    let mut session = zoomed_in();
    session.fit_project();
    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    assert!(session.flying(), "the wheel meets a fly in flight");
    session.wheel(500.0, 0.0, 0.0, -50.0, true);
    let put = view(&session);

    at(1_000.0 + FLY_MS * 2.0);
    session.advance_fly();
    assert_eq!(view(&session), put, "the fly did not take the view back");
    assert!(!session.flying());
}

#[test]
fn a_drag_mid_fly_leaves_the_view_where_the_drag_put_it() {
    let mut session = zoomed_in();
    session.fit_project();
    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    assert!(session.flying(), "the drag meets a fly in flight");
    // Right of the chart's bars, on no bar: a press there pans.
    session.pointer_down(990.0, 395.0, false);
    assert!(!session.flying(), "the press ends the fly at once");
    let held = view(&session);
    // A frame while the press holds the chart does not move the view under the pointer.
    at(1_000.0 + FLY_MS * 0.75);
    session.advance_fly();
    assert_eq!(view(&session), held);
    session.pointer_move(900.0, 395.0);
    let put = view(&session);

    at(1_000.0 + FLY_MS * 2.0);
    session.advance_fly();
    assert_eq!(view(&session), put, "the fly did not take the view back");
}

#[test]
fn a_wheel_that_moves_nothing_still_ends_the_fly() {
    let mut session = zoomed_in();
    session.fit_project();
    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    let mid = view(&session);
    assert!(session.flying());
    session.wheel(500.0, 0.0, 0.0, 0.0, false);
    assert!(
        !session.flying(),
        "the wheel ended it, though the view did not move"
    );
    assert_eq!(view(&session), mid);
}

#[test]
fn reduced_motion_turned_on_mid_fly_lands_at_the_target() {
    let (_, target) = start_and_target();
    let mut session = zoomed_in();
    session.fit_project();
    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    assert!(session.flying());
    session.set_reduced_motion(true);
    session.advance_fly();
    assert_eq!(view(&session), target);
    assert!(!session.flying());
}

#[test]
fn a_key_ends_the_fly_where_it_is() {
    let mut session = zoomed_in();
    session.fit_project();
    at(1_000.0 + FLY_MS / 2.0);
    session.advance_fly();
    let mid = view(&session);
    assert!(session.flying(), "the key meets a fly in flight");
    session.cancel_fly();

    at(1_000.0 + FLY_MS * 2.0);
    session.advance_fly();
    assert_eq!(view(&session), mid);
}

#[test]
fn reduced_motion_jumps_to_the_target_at_once() {
    let (_, target) = start_and_target();
    let mut session = zoomed_in();
    session.set_reduced_motion(true);
    assert!(session.fit_project());
    assert_eq!(view(&session), target, "t = 0 is the target");
    assert!(!session.flying());
}

#[test]
fn find_flies_to_a_task_off_screen() {
    let mut session = zoomed_in();
    // The last task by id, found by its own name (a name may repeat; the first hit that is it).
    let far = session.meta().len() as u32 - 1;
    let name = session.meta()[far as usize].name.clone();
    let hits = session.find(&name);
    let k = hits
        .iter()
        .position(|&t| t == far)
        .expect("the name finds it");
    let start = view(&session);
    let rows = session.timeline().viewport.scroll_y;
    assert_eq!(session.find_go(k as u32), Some(far));
    assert!(session.flying(), "the reveal flies");
    assert_eq!(view(&session), start, "t = 0 is where the view was");
    assert_eq!(session.timeline().viewport.scroll_y, rows);

    at(1_000.0 + FLY_MS);
    session.advance_fly();
    assert_ne!(
        session.timeline().viewport.scroll_y,
        rows,
        "the rows landed"
    );
    assert!(!session.flying());
}
