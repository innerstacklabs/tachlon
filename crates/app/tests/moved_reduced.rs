//! The "what moved" note under reduced motion (gs#516, story S5c of gs#429): with the page's
//! reduced-motion flag on, the report's alpha is 1 for `HOLD_MS` and then 0, with no ramp; the
//! hold rules are unchanged. Time is a fake clock the test sets, so each reading is exact. The
//! alpha is read from `status_json`, the way the page reads it.

use app::{Loaded, RenderFacts, Session, edit::Edit, load_bytes};
use engine::TaskId;
use import::ProjectSelection;
use std::cell::Cell;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The task whose longer duration moves 8 others (see `moved.rs`), and the duration that moves
/// them (its own is 2400).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;

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

fn loaded() -> Loaded {
    load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads")
}

fn ledger() -> Session {
    at(0.0);
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = loaded();
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 600.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin)
}

fn set_duration(s: &mut Session, minutes: i64) {
    s.edit(Edit::SetDuration {
        task: EDITED,
        minutes,
    })
    .expect("applies");
}

/// The report's alpha as the page reads it from the status string, at the clock's time.
fn alpha(s: &Session) -> f64 {
    let json = s.status_json(&RenderFacts::default());
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    value["moved"]["alpha"].as_f64().expect("a report is shown")
}

/// The report's alpha with the clock set to `ms`.
fn alpha_at(s: &Session, ms: f64) -> f64 {
    at(ms);
    alpha(s)
}

#[test]
fn with_reduced_motion_the_note_steps_from_full_to_gone() {
    let mut s = ledger();
    s.set_reduced_motion(true);
    set_duration(&mut s, LONGER);
    assert_eq!(alpha_at(&s, 0.0), 1.0);
    assert_eq!(alpha_at(&s, 3900.0), 1.0);
    assert_eq!(alpha_at(&s, 4000.0), 1.0, "the hold ends after 4 s");
    assert_eq!(alpha_at(&s, 4200.0), 0.0, "no ramp: 0, not 0.5");
    assert_eq!(alpha_at(&s, 4400.0), 0.0);

    // The same edit with the flag off is still fading at 4.2 s.
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    assert!((alpha_at(&s, 4200.0) - 0.5).abs() < 1e-9);
}

#[test]
fn the_flag_toggled_mid_report_follows_the_new_value() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    assert!(
        (alpha_at(&s, 4200.0) - 0.5).abs() < 1e-9,
        "motion on: fading"
    );
    s.set_reduced_motion(true);
    assert_eq!(alpha(&s), 0.0, "turned on: the fade is gone");
    s.set_reduced_motion(false);
    assert!((alpha(&s) - 0.5).abs() < 1e-9, "turned off: fading again");

    // Turned on before the hold ends, a report at 3.9 s still shows in full, then drops.
    at(3900.0);
    s.set_reduced_motion(true);
    assert_eq!(alpha(&s), 1.0);
    assert_eq!(alpha_at(&s, 4050.0), 0.0);
}

#[test]
fn the_hold_still_works_with_reduced_motion() {
    // Held past the hold: full whatever the age.
    let mut s = ledger();
    s.set_reduced_motion(true);
    set_duration(&mut s, LONGER);
    at(3900.0);
    s.set_moved_hold(true);
    for ms in [4000.0, 4200.0, 4500.0, 60_000.0] {
        assert_eq!(alpha_at(&s, ms), 1.0, "held at {ms} ms");
    }

    // A release keeps at least a second of full opacity, then the note is gone with no ramp.
    at(10_000.0);
    s.set_moved_hold(false);
    assert_eq!(alpha(&s), 1.0);
    assert_eq!(alpha_at(&s, 10_999.0), 1.0, "a second of full opacity");
    assert_eq!(alpha_at(&s, 11_000.0), 1.0);
    assert_eq!(alpha_at(&s, 11_200.0), 0.0, "no fade after the release");

    // A hold after the note is gone does nothing.
    s.set_moved_hold(true);
    assert_eq!(alpha_at(&s, 12_000.0), 0.0);
}
