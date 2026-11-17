//! The slide state of a session (gs#484, story S3.4 of gs#427): `advance_motion` starts, steps and
//! snaps a slide, and `display_dates` is the copy it draws while one runs. Time is a fake clock
//! the test sets, so a frame is exact.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use engine::{Moved, TaskDates, TaskId};
use import::ProjectSelection;
use std::cell::Cell;
use timeline::{Timeline, slide};

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

/// The task whose longer duration moves 8 others (see `moved.rs`).
const EDITED: TaskId = TaskId(16);
const LONGER: i64 = 4800;

thread_local! {
    /// The fake clock. Each test runs on its own thread, so each has its own time.
    static NOW: Cell<f64> = const { Cell::new(0.0) };
    /// How far one read of the clock moves it.
    static STEP: Cell<f64> = const { Cell::new(0.0) };
}

fn clock() -> f64 {
    NOW.with(|now| {
        let t = now.get();
        now.set(t + STEP.with(Cell::get));
        t
    })
}

fn set_clock(ms: f64) {
    NOW.with(|now| now.set(ms));
}

fn ledger() -> Session {
    set_clock(0.0);
    STEP.with(|step| step.set(0.0));
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads");
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

fn set_duration(s: &mut Session, task: TaskId, minutes: i64) {
    s.edit(Edit::SetDuration { task, minutes })
        .expect("applies");
}

/// The edit that moves the others, and the first frame that sees it.
fn edited_and_started(s: &mut Session) {
    set_duration(s, EDITED, LONGER);
    s.advance_motion();
}

fn same_slice(s: &Session) -> bool {
    s.display_dates().as_ptr() == s.solution().dates.as_ptr()
        && s.display_dates().len() == s.solution().dates.len()
}

fn assert_equals_compute(s: &Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

/// The edited task and the others the report lists, with their old dates.
fn moved_tasks(s: &Session) -> Vec<Moved> {
    let r = s.move_report().expect("the edit moved others");
    r.edited.iter().chain(&r.was).copied().collect()
}

fn early(d: &TaskDates) -> (i64, i64) {
    (d.es, d.ef)
}

#[test]
fn idle_frames_draw_the_solution_slice_itself() {
    let mut s = ledger();
    assert!(same_slice(&s));
    s.advance_motion();
    assert!(same_slice(&s), "no report, no copy");
    set_duration(&mut s, EDITED, LONGER);
    assert!(same_slice(&s), "an edit alone makes no copy: a frame does");
    assert_eq!(s.slide_copy_ms(), None);
}

#[test]
fn a_slide_draws_the_old_dates_at_zero_a_blend_at_the_middle_and_the_new_at_the_end() {
    let mut s = ledger();
    edited_and_started(&mut s);
    let moved = moved_tasks(&s);
    assert!(moved.len() >= 2, "the edit moved others");
    let total = slide::total_ms(moved.len());
    assert!(!same_slice(&s), "a slide draws a copy");

    // t = 0: every moved task is where it was; the schedule columns are the solution's.
    for m in &moved {
        let d = &s.display_dates()[m.task.0 as usize];
        assert_eq!(early(d), (m.was_es, m.was_ef), "task {} at t = 0", m.task.0);
        let new = &s.solution().dates[m.task.0 as usize];
        assert_eq!((d.ls, d.lf, d.float), (new.ls, new.lf, new.float));
    }

    // The midpoint: strictly between the old dates and the new.
    set_clock(total / 2.0);
    s.advance_motion();
    assert!(!same_slice(&s));
    for m in &moved {
        let d = early(&s.display_dates()[m.task.0 as usize]);
        let new = early(&s.solution().dates[m.task.0 as usize]);
        assert_ne!(
            d,
            (m.was_es, m.was_ef),
            "task {} left its old dates",
            m.task.0
        );
        assert_ne!(d, new, "task {} has not arrived", m.task.0);
    }

    // One millisecond before the end it still runs; at the end the copy is dropped.
    set_clock(total - 1.0);
    s.advance_motion();
    assert!(!same_slice(&s));
    set_clock(total);
    s.advance_motion();
    assert!(same_slice(&s), "the slide ended");
    assert_equals_compute(&s);
}

#[test]
fn the_slide_starts_on_the_first_frame_that_sees_the_report() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert_eq!(s.move_report().expect("others moved").at, 0.0);
    // The edit was slow: the first frame comes long after the report was made.
    set_clock(5000.0);
    s.advance_motion();
    assert!(!same_slice(&s), "a slide started");
    let moved = moved_tasks(&s);
    for m in &moved {
        let d = early(&s.display_dates()[m.task.0 as usize]);
        assert_eq!(d, (m.was_es, m.was_ef), "the slide begins at the frame");
    }
    set_clock(5000.0 + slide::total_ms(moved.len()) - 1.0);
    s.advance_motion();
    assert!(!same_slice(&s), "the whole time is still ahead");
}

#[test]
fn a_moved_task_with_no_row_is_not_copied() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    let moved = moved_tasks(&s);
    // Collapse the first summary above a moved task, so that task has no row.
    let outline = s.outline().expect("an outline");
    let hidden = moved
        .iter()
        .find_map(|m| {
            let parent = outline.parent(m.task)?;
            outline.is_summary(parent).then_some((m.task, parent))
        })
        .expect("a moved task sits under a summary");
    assert!(s.toggle(hidden.1), "the summary collapses");
    assert!(s.timeline().row_of_task(hidden.0.0).is_none());
    s.advance_motion();
    set_clock(slide::total_ms(moved.len()) / 2.0);
    s.advance_motion();
    let shown: Vec<&Moved> = moved
        .iter()
        .filter(|m| s.timeline().row_of_task(m.task.0).is_some())
        .collect();
    assert!(!shown.is_empty(), "a moved task still has a row");
    assert!(!same_slice(&s), "the tasks with a row slide");
    let new = s.solution().dates[hidden.0.0 as usize];
    assert_eq!(
        s.display_dates()[hidden.0.0 as usize],
        new,
        "a task with no row keeps the solution's dates"
    );
    for m in shown {
        assert_ne!(
            early(&s.display_dates()[m.task.0 as usize]),
            early(&s.solution().dates[m.task.0 as usize]),
            "task {} slides",
            m.task.0
        );
    }
}

#[test]
fn a_report_whose_moved_tasks_all_lack_a_row_starts_no_slide() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    let moved = moved_tasks(&s);
    assert!(s.collapse_to_level(1), "the rows collapse");
    assert!(
        moved
            .iter()
            .all(|m| s.timeline().row_of_task(m.task.0).is_none()),
        "every moved task is hidden"
    );
    // A clock that moves on every read times any copy, so a copy of nothing shows in `slide_copy_ms`.
    STEP.with(|step| step.set(0.25));
    s.advance_motion();
    assert!(same_slice(&s), "nothing to slide, so no copy");
    assert_eq!(s.slide_copy_ms(), None);
}

#[test]
fn reduced_motion_shows_the_final_dates_on_the_first_frame() {
    let mut s = ledger();
    s.set_reduced_motion(true);
    edited_and_started(&mut s);
    assert!(s.move_report().is_some());
    assert!(same_slice(&s));
    assert_equals_compute(&s);
}

#[test]
fn a_report_made_with_reduced_motion_on_never_plays_later() {
    let mut s = ledger();
    s.set_reduced_motion(true);
    set_duration(&mut s, EDITED, LONGER);
    // No frame ran while it was on: turning it off must not let the report play.
    s.set_reduced_motion(false);
    s.advance_motion();
    assert!(same_slice(&s));

    // A frame ran while it was on, then it went off.
    set_duration(&mut s, EDITED, LONGER + 480);
    s.set_reduced_motion(true);
    s.advance_motion();
    s.set_reduced_motion(false);
    s.advance_motion();
    assert!(same_slice(&s));

    // And the next edit slides again.
    set_duration(&mut s, EDITED, LONGER);
    s.advance_motion();
    assert!(!same_slice(&s));
}

#[test]
fn turning_reduced_motion_on_drops_the_running_slide() {
    let mut s = ledger();
    edited_and_started(&mut s);
    assert!(!same_slice(&s));
    s.set_reduced_motion(true);
    assert!(same_slice(&s));
}

#[test]
fn a_second_edit_mid_slide_snaps_then_restarts_from_the_new_old_dates() {
    let mut s = ledger();
    edited_and_started(&mut s);
    let first = moved_tasks(&s).len();
    set_clock(100.0);
    s.advance_motion();
    assert!(!same_slice(&s));

    // The second edit: a shorter duration, so the same tasks move back.
    let before = s.solution().dates.clone();
    set_duration(&mut s, EDITED, 3600);
    let moved = moved_tasks(&s);
    set_clock(150.0);
    s.advance_motion();
    assert!(!same_slice(&s), "the second edit slides");
    for m in &moved {
        let d = early(&s.display_dates()[m.task.0 as usize]);
        assert_eq!(d, (m.was_es, m.was_ef), "restarted from the new was");
        assert_eq!(
            d,
            early(&before[m.task.0 as usize]),
            "the old dates are the first edit's"
        );
    }
    // It runs a whole slide from the frame that saw it, not from the first edit.
    set_clock(150.0 + slide::total_ms(moved.len()) - 1.0);
    s.advance_motion();
    assert!(!same_slice(&s));
    set_clock(150.0 + slide::total_ms(moved.len()));
    s.advance_motion();
    assert!(same_slice(&s));
    assert!(first >= 2);
    assert_equals_compute(&s);
}

#[test]
fn an_insert_mid_slide_snaps() {
    let mut s = ledger();
    edited_and_started(&mut s);
    set_clock(100.0);
    s.advance_motion();
    assert!(!same_slice(&s), "the slide runs");
    let tasks = s.solution().dates.len();

    // An insert replaces the solution and makes no report.
    s.insert_below_selection().expect("inserts");
    assert!(s.move_report().is_none());
    assert_eq!(s.solution().dates.len(), tasks + 1);
    set_clock(110.0);
    s.advance_motion();
    assert!(same_slice(&s), "the slide snapped to the new solution");
    assert_equals_compute(&s);
}

#[test]
fn an_undo_mid_slide_snaps_and_slides_back() {
    let mut s = ledger();
    edited_and_started(&mut s);
    set_clock(100.0);
    s.advance_motion();
    assert_eq!(s.undo(), Ok(true));
    set_clock(110.0);
    s.advance_motion();
    let moved = moved_tasks(&s);
    for m in &moved {
        assert_eq!(
            early(&s.display_dates()[m.task.0 as usize]),
            (m.was_es, m.was_ef),
            "undo restarts from the dates it left"
        );
    }
    assert_equals_compute(&s);
}

#[test]
fn a_drag_release_starts_nothing() {
    let (mut s, _task, _es, (x, y)) = common::setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 60.0, y);
    s.apply_drag();
    s.advance_motion();
    assert!(same_slice(&s), "a frame of the drag");
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    s.pointer_up();
    let r = s.move_report().expect("the drag moved others");
    assert!(!r.slide && r.others > 0);
    s.advance_motion();
    assert!(same_slice(&s), "a release starts no slide");
    assert_equals_compute(&s);
}

#[test]
fn the_solution_is_the_full_solve_before_during_and_after_a_slide() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    assert_equals_compute(&s);
    s.advance_motion();
    assert!(!same_slice(&s));
    assert_equals_compute(&s);
    set_clock(120.0);
    s.advance_motion();
    assert!(!same_slice(&s));
    assert_equals_compute(&s);
    set_clock(10_000.0);
    s.advance_motion();
    assert!(same_slice(&s));
    assert_equals_compute(&s);
}

#[test]
fn the_copy_is_timed_with_the_session_clock() {
    let mut s = ledger();
    set_duration(&mut s, EDITED, LONGER);
    STEP.with(|step| step.set(0.25));
    s.advance_motion();
    assert!(!same_slice(&s));
    assert_eq!(s.slide_copy_ms(), Some(0.25));
}
