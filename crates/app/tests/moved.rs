//! The move report of an edit (gs#424, story S2 of #309): what `Session::edit`, undo and redo
//! say moved besides the edited task, and the `moved` field of `status_json`.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, RenderFacts, Session, edit::Edit, load_bytes};
use common::{clock, xer_text::rewrite};
use engine::{DepId, LinkType, Moved, TaskId};
use import::ProjectSelection;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The fixture with no actual dates: an activity with actuals is fixed at them, and these tests
/// measure what an edit moves.
fn not_started(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let text = rewrite(&text, "TASK", "act_start_date", |_, _| Some(String::new()));
    rewrite(&text, "TASK", "act_end_date", |_, _| Some(String::new())).into_bytes()
}

/// The 43-row fixture the options page used.
fn ledger() -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(
        &not_started(LEDGER_XER),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("the file loads");
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

/// The task whose longer duration moves 8 others, and the duration that moves the finish 7 days
/// (measured on this fixture: its own duration is 2400 minutes).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;

fn assert_equals_compute(s: &Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

fn set_duration(s: &mut Session, task: TaskId, minutes: i64) {
    s.edit(Edit::SetDuration { task, minutes })
        .expect("applies");
}

#[test]
fn a_duration_edit_reports_the_others_and_the_finish() {
    let mut s = ledger();
    assert_eq!(s.schedule().tasks().len(), 43);
    assert!(s.move_report().is_none());
    set_duration(&mut s, EDITED, LONGER);
    let r = s.move_report().expect("others moved");
    assert_eq!(
        (r.others, r.finish_days, r.slide),
        (8, 7, true),
        "{}",
        r.text
    );
    assert_eq!(r.text, "8 tasks moved · finish +7 days");
    assert_equals_compute(&s);
}

#[test]
fn the_count_leaves_out_the_edited_task() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    let r = s.move_report().expect("others moved");
    // The engine's list holds the edited task too: the report must not.
    assert!(r.was.iter().all(|m| m.task != EDITED));
    assert!(r.edited.is_some(), "the edited task is reported apart");
    assert_eq!(r.others as usize, r.was.len());
    assert_eq!(r.others, 8, "{}", r.text);
}

#[test]
fn a_duration_edit_reports_the_edited_tasks_old_dates() {
    let mut s = ledger();
    let start = early(&s);
    set_duration(&mut s, EDITED, LONGER);
    let now = early(&s);
    let r = s.move_report().expect("others moved");
    let old = start[EDITED.0 as usize];
    assert_ne!(
        old.1, now[EDITED.0 as usize].1,
        "the edit did not move its ef"
    );
    assert_eq!(r.edited, Some(was(EDITED, old)), "{}", r.text);
}

#[test]
fn one_task_says_task_and_an_unmoved_finish_has_no_clause() {
    let mut s = ledger();
    // Activity A1-21 of the fixture has one successor that moves, and the finish stays on its day.
    set_duration(&mut s, TaskId(12), 4320 + 7 * 1440);
    let r = s.move_report().expect("one other moved");
    assert_eq!(r.text, "1 task moved");
    assert_eq!(r.finish_days, 0);
}

#[test]
fn an_edit_that_moves_only_itself_makes_no_report() {
    let mut s = ledger();
    set_duration(&mut s, TaskId(0), 7 * 1440);
    assert!(s.move_report().is_none());
    assert_equals_compute(&s);
}

#[test]
fn a_later_change_clears_the_report() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert!(s.move_report().is_some());
    s.edit(Edit::Rename {
        task: EDITED,
        name: "Renamed".into(),
    })
    .unwrap();
    assert!(s.move_report().is_none());
}

fn early(s: &Session) -> Vec<(i64, i64)> {
    s.solution().dates.iter().map(|d| (d.es, d.ef)).collect()
}

#[expect(
    clippy::similar_names,
    reason = "was_es and was_ef are the two ends of the old span"
)]
fn was(task: TaskId, (was_es, was_ef): (i64, i64)) -> Moved {
    Moved {
        task,
        was_es,
        was_ef,
    }
}

fn assert_was(r: &[Moved], before: &[(i64, i64)]) {
    assert_ne!(r, []);
    for m in r {
        assert_eq!(
            (m.was_es, m.was_ef),
            before[m.task.0 as usize],
            "task {}",
            m.task.0
        );
    }
}

#[test]
fn undo_and_redo_report_with_was_and_now_swapped() {
    let mut s = ledger();
    let start = early(&s);
    set_duration(&mut s, EDITED, LONGER);
    let edited = early(&s);
    let seq = s.move_report().unwrap().seq;
    assert_was(&s.move_report().unwrap().was, &start);

    assert!(s.undo().unwrap());
    let r = s.move_report().expect("undo moved the others back");
    assert_eq!((r.others, r.finish_days, r.slide), (8, -7, true));
    assert_eq!(r.text, "8 tasks moved · finish -7 days");
    assert_ne!(r.seq, seq);
    assert_was(&r.was, &edited);
    assert_eq!(r.edited, Some(was(EDITED, edited[EDITED.0 as usize])));
    assert_eq!(early(&s), start);
    assert_equals_compute(&s);

    assert!(s.redo().unwrap());
    let r = s.move_report().expect("redo moved them again");
    assert_eq!((r.others, r.finish_days), (8, 7));
    assert_was(&r.was, &start);
    assert_eq!(r.edited, Some(was(EDITED, start[EDITED.0 as usize])));
    assert_eq!(early(&s), edited);
    assert_equals_compute(&s);
}

/// A link whose tasks sit apart: finish-to-start from `PRED` to `SUCC` pushes a cone of tasks and
/// the finish later (measured on this fixture; the tests below also count the moved tasks from the
/// early dates, so the figures are not the only check).
const PRED: TaskId = TaskId(13);
const SUCC: TaskId = TaskId(15);

fn add_link(s: &mut Session) {
    s.edit(Edit::AddLink {
        pred: PRED,
        succ: SUCC,
        kind: LinkType::FS,
        lag: 0,
    })
    .expect("applies");
}

/// How many tasks have other early dates in `after` than in `before`.
fn moved_count(before: &[(i64, i64)], after: &[(i64, i64)]) -> usize {
    before.iter().zip(after).filter(|(a, b)| a != b).count()
}

#[test]
fn a_link_edit_reports_the_others_and_the_finish() {
    let mut s = ledger();
    let start = early(&s);
    assert!(s.move_report().is_none());
    add_link(&mut s);
    let n = moved_count(&start, &early(&s));
    let r = s.move_report().expect("a link moved tasks");
    // No task is the edited one: every task that moved is counted and listed.
    assert_eq!(
        (r.others as usize, r.finish_days, r.slide, r.edited),
        (n, 5, true, None),
        "{}",
        r.text
    );
    assert_eq!(
        r.text,
        format!("{n} tasks moved · finish +5 days · critical 2 joined, 3 left")
    );
    assert_eq!(r.was.len(), n);
    assert_was(&r.was, &start);
    assert_equals_compute(&s);
}

#[test]
fn removing_a_link_reports_what_moved_back() {
    let mut s = ledger();
    let start = early(&s);
    add_link(&mut s);
    let linked = early(&s);
    let dep = DepId(u32::try_from(s.schedule().dependencies().len() - 1).unwrap());
    s.edit(Edit::RemoveLink { dep }).expect("applies");
    let r = s.move_report().expect("the tasks moved back");
    assert_eq!(r.others, moved_count(&linked, &start) as u32);
    assert_eq!((r.finish_days, r.slide, r.edited), (-5, true, None));
    assert_was(&r.was, &linked);
    assert_eq!(early(&s), start);
    assert_equals_compute(&s);
}

#[test]
fn undo_of_a_link_edit_reports_with_was_and_now_swapped() {
    let mut s = ledger();
    let start = early(&s);
    add_link(&mut s);
    let linked = early(&s);
    let seq = s.move_report().unwrap().seq;

    assert!(s.undo().unwrap());
    let r = s.move_report().expect("undo moved the tasks back");
    assert_eq!(r.others, moved_count(&linked, &start) as u32);
    assert_eq!((r.finish_days, r.slide, r.edited), (-5, true, None));
    assert_ne!(r.seq, seq);
    assert_was(&r.was, &linked);
    assert_eq!(early(&s), start);
    assert_equals_compute(&s);

    assert!(s.redo().unwrap());
    let r = s.move_report().expect("redo moved them again");
    assert_eq!(r.others, moved_count(&start, &linked) as u32);
    assert_eq!((r.finish_days, r.slide), (5, true));
    assert_was(&r.was, &start);
    assert_eq!(early(&s), linked);
    assert_equals_compute(&s);
}

#[test]
fn the_alpha_holds_then_falls_to_zero() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    let r = s.move_report().unwrap();
    assert_eq!(r.alpha(r.at, false, false), 1.0);
    assert_eq!(r.alpha(r.at + 4000.0, false, false), 1.0);
    assert!((r.alpha(r.at + 4200.0, false, false) - 0.5).abs() < 1e-9);
    assert_eq!(r.alpha(r.at + 4400.0, false, false), 0.0);
    assert_eq!(r.alpha(r.at + 9000.0, false, false), 0.0);
}

/// The text a status string holds from the `moved` key to the `slide` key after it.
fn moved_field(json: &str) -> &str {
    let at = json.find(r#","moved":"#).expect("status has moved");
    let end = json.rfind(r#","slide":"#).expect("status has slide");
    &json[at..end]
}

#[test]
fn status_moved_is_null_or_the_report_and_costs_bounded_bytes() {
    let facts = RenderFacts::default();
    let mut s = ledger();
    let json = s.status_json(&facts);
    let null_bytes = moved_field(&json).len();
    assert_eq!(moved_field(&json), r#","moved":null"#);

    set_duration(&mut s, EDITED, LONGER);
    let json = s.status_json(&facts);
    let set_bytes = moved_field(&json).len();
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    let r = s.move_report().unwrap();
    assert_eq!(value["moved"]["seq"], r.seq);
    assert_eq!(value["moved"]["text"], "8 tasks moved · finish +7 days");
    assert_eq!(value["moved"]["others"], 8);
    assert_eq!(value["moved"]["finish_days"], 7);
    assert_eq!(value["moved"]["alpha"], 1.0);
    assert_eq!(value["moved"]["slide"], true);
    // The plan budgeted 5 bytes (null) and 80 bytes (set); the key alone is 9 bytes, so those
    // cannot hold. These are the measured sizes, so a field that grows fails here.
    assert_eq!(null_bytes, 13);
    assert!(
        set_bytes <= 125,
        "{set_bytes} bytes: {}",
        moved_field(&json)
    );
}

/// The text a status string holds from the `slide` key to its closing brace.
fn slide_field(json: &str) -> &str {
    let at = json.rfind(r#","slide":"#).expect("status has slide");
    &json[at..json.len() - 1]
}

#[test]
fn status_slide_is_idle_with_a_null_copy_time_and_costs_bounded_bytes() {
    let facts = RenderFacts::default();
    let mut s = ledger();
    let json = s.status_json(&facts);
    assert_eq!(
        slide_field(&json),
        r#","slide":{"active":false,"copyMs":null}"#
    );
    // The plan budgeted at most 45 bytes for a null copy time; this is the measured size.
    assert_eq!(slide_field(&json).len(), 39);
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    assert_eq!(value["slide"]["active"], false);
    assert!(value["slide"]["copyMs"].is_null());

    // An edit alone starts nothing: a frame does.
    set_duration(&mut s, EDITED, LONGER);
    let json = s.status_json(&facts);
    assert_eq!(
        slide_field(&json),
        r#","slide":{"active":false,"copyMs":null}"#
    );
    s.advance_motion();
    let value: serde_json::Value = serde_json::from_str(&s.status_json(&facts)).expect("JSON");
    assert_eq!(value["slide"]["active"], true);
    assert!(value["slide"]["copyMs"].is_number());

    // Reduced motion drops the slide; the copy time stays.
    s.set_reduced_motion(true);
    let value: serde_json::Value = serde_json::from_str(&s.status_json(&facts)).expect("JSON");
    assert_eq!(value["slide"]["active"], false);
    assert!(value["slide"]["copyMs"].is_number());
}
