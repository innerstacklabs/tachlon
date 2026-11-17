//! The hold on the "what moved" note (gs#490, story S5b of gs#429): while the pointer is over the
//! note the report stays at full opacity, and on release it keeps the time it had left and at
//! least `RESUME_MIN_MS` of full opacity. Time is a fake clock the test sets, so each reading is
//! exact. The alpha is read from `status_json`, the way the page reads it.

use app::{Loaded, RenderFacts, Session, edit::Edit, load_bytes};
use engine::TaskId;
use import::ProjectSelection;
use std::cell::Cell;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The task whose longer duration moves 8 others (see `moved.rs`), and two durations that move
/// others each time it changes (its own is 2400).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;
const LONGEST: i64 = 7200;

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
fn a_held_report_stays_full_past_the_hold_and_the_fade() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    assert_eq!(alpha_at(&s, 3900.0), 1.0);
    s.set_moved_hold(true);
    for ms in [4000.0, 4200.0, 4500.0, 60_000.0] {
        assert_eq!(alpha_at(&s, ms), 1.0, "held at {ms} ms");
    }
}

#[test]
fn a_release_keeps_the_time_left_and_at_least_a_second() {
    // Entered at 3.9 s with 100 ms left: the second of full opacity is the longer.
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    at(3900.0);
    s.set_moved_hold(true);
    at(10_000.0);
    s.set_moved_hold(false);
    assert_eq!(alpha(&s), 1.0);
    assert_eq!(alpha_at(&s, 10_999.0), 1.0, "a second of full opacity");
    assert_eq!(alpha_at(&s, 11_000.0), 1.0);
    assert!((alpha_at(&s, 11_200.0) - 0.5).abs() < 1e-9, "then the fade");
    assert_eq!(alpha_at(&s, 11_400.0), 0.0);

    // Entered at 1 s with 3 s left: the time left is the longer.
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    at(1000.0);
    s.set_moved_hold(true);
    at(5000.0);
    s.set_moved_hold(false);
    assert_eq!(alpha_at(&s, 7999.0), 1.0, "the 3 s it had left");
    assert!((alpha_at(&s, 8200.0) - 0.5).abs() < 1e-9);
    assert_eq!(alpha_at(&s, 8400.0), 0.0);
}

#[test]
fn a_hold_during_the_fade_restores_full_opacity() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    assert!((alpha_at(&s, 4200.0) - 0.5).abs() < 1e-9, "fading");
    s.set_moved_hold(true);
    assert_eq!(alpha(&s), 1.0);
    assert_eq!(alpha_at(&s, 5000.0), 1.0);
    at(5100.0);
    s.set_moved_hold(false);
    assert_eq!(alpha_at(&s, 6099.0), 1.0, "a second after the release");
    assert_eq!(alpha_at(&s, 6500.0), 0.0);
}

#[test]
fn a_hold_with_no_report_or_a_faded_one_does_nothing() {
    let mut s = ledger();
    s.set_moved_hold(true);
    s.set_moved_hold(false);
    set_duration(&mut s, LONGER);
    assert!((alpha_at(&s, 4200.0) - 0.5).abs() < 1e-9, "not held");

    at(9000.0);
    s.set_moved_hold(true);
    assert_eq!(alpha(&s), 0.0, "a faded report is not brought back");
}

#[test]
fn a_new_report_while_held_stays_held() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    at(100.0);
    s.set_moved_hold(true);
    at(500.0);
    set_duration(&mut s, LONGEST);
    let r = s.move_report().expect("a new report");
    assert_eq!(r.at, 500.0);
    assert_eq!(alpha_at(&s, 20_000.0), 1.0, "still held");
    at(21_000.0);
    s.set_moved_hold(false);
    // The new report is fresh: it has its whole 4 s left, not what the first one had.
    assert_eq!(alpha_at(&s, 24_999.0), 1.0);
    assert!((alpha_at(&s, 25_200.0) - 0.5).abs() < 1e-9);
}

#[test]
fn clearing_the_report_clears_the_hold() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    s.set_moved_hold(true);
    s.edit(Edit::Rename {
        task: EDITED,
        name: "Renamed".into(),
    })
    .unwrap();
    assert!(s.move_report().is_none());
    // The next report is not held.
    at(100.0);
    set_duration(&mut s, LONGEST);
    assert!((alpha_at(&s, 4300.0) - 0.5).abs() < 1e-9);
    assert_eq!(alpha_at(&s, 4500.0), 0.0);
}

#[test]
fn opening_a_file_ends_the_hold() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    s.set_moved_hold(true);
    let next = loaded();
    let mut fresh = s
        .reopen(
            next.schedule,
            next.solution,
            next.order,
            next.meta,
            Some(next.outline),
            next.data_date,
            next.origin,
        )
        .expect("the outline matches its schedule");
    at(100.0);
    set_duration(&mut fresh, LONGER);
    assert!((alpha_at(&fresh, 4300.0) - 0.5).abs() < 1e-9, "not held");
}

#[test]
fn a_repeat_of_the_state_changes_nothing() {
    let mut s = ledger();
    set_duration(&mut s, LONGER);
    // A release with no hold leaves the report where it was.
    at(500.0);
    s.set_moved_hold(false);
    assert_eq!(s.move_report().unwrap().at, 0.0);
    // A second entry keeps the first one's time.
    at(1000.0);
    s.set_moved_hold(true);
    at(3000.0);
    s.set_moved_hold(true);
    at(5000.0);
    s.set_moved_hold(false);
    assert_eq!(
        alpha_at(&s, 7999.0),
        1.0,
        "3 s left from the first entry at 1 s"
    );
    assert!((alpha_at(&s, 8200.0) - 0.5).abs() < 1e-9);
}

/// The text a status string holds from the `moved` key to the `slide` key after it.
fn moved_field(json: &str) -> &str {
    let at = json.find(r#","moved":"#).expect("status has moved");
    let end = json.rfind(r#","slide":"#).expect("status has slide");
    &json[at..end]
}

#[test]
fn the_moved_json_keeps_its_byte_budget_held_or_not() {
    let facts = RenderFacts::default();
    let mut s = ledger();
    assert_eq!(moved_field(&s.status_json(&facts)).len(), 13);
    set_duration(&mut s, LONGER);
    assert_eq!(moved_field(&s.status_json(&facts)).len(), 113);
    at(4300.0);
    let unheld = moved_field(&s.status_json(&facts)).len();
    at(3900.0);
    s.set_moved_hold(true);
    at(4300.0);
    let json = s.status_json(&facts);
    assert_eq!(moved_field(&json).len(), unheld, "{}", moved_field(&json));
    assert_eq!(unheld, 113);
    assert!(moved_field(&json).contains(r#""alpha":1.000"#));
}
