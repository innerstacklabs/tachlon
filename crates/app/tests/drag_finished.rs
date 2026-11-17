//! A bar drag on a finished task (gs#895): the bar does not move, and the release says why.

use crate::common;

use app::{RenderFacts, Session, edit::Edit, meta::TaskMeta};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;

fn at(day: i64, hour: i64) -> Instant {
    day * DAY + hour * HOUR
}

/// Two unlinked tasks; the first is finished. Returns the session and the canvas point on the
/// first task's bar (one pixel is ten minutes).
fn finished_session() -> (Session, (f64, f64)) {
    session_with(Some(at(0, 17)))
}

fn session_with(actual_finish: Option<Instant>) -> (Session, (f64, f64)) {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in 0..2 {
        schedule
            .add_task(Task {
                duration: 8 * HOUR,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("the calendar belongs to the schedule");
    }
    let solution = schedule.compute().expect("computes");
    let mut timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    timeline.viewport.px_per_minute = 0.1;
    timeline.viewport.t0 = 0.0;
    let x = timeline.viewport.x_of(at(0, 9) as f64);
    let y = timeline.viewport.row_top(0) + timeline.viewport.row_height / 2.0;
    let mut s = Session::new(
        schedule,
        solution,
        timeline,
        vec![
            TaskMeta {
                name: "Space planning".into(),
                ..TaskMeta::default()
            },
            TaskMeta::default(),
        ],
        clock,
    );
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(at(0, 8)),
        actual_finish,
        remaining: if actual_finish.is_some() {
            None
        } else {
            Some(8 * HOUR)
        },
    })
    .expect("applies");
    assert_eq!(
        s.timeline().hit_test(&s.solution().dates, x, y),
        Some(0),
        "the press must land on the bar"
    );
    (s, (x, y))
}

const NOTE: &str = "Space planning is finished: its dates are its actuals. Change Actual finish in the table to move it.";

#[test]
fn a_drag_of_a_finished_task_moves_nothing_and_the_release_says_why() {
    let (mut s, (x, y)) = finished_session();
    let tasks = s.schedule().tasks().to_vec();
    let solution = s.solution().clone();
    let (revision, stamp) = (s.revision(), s.stamp());

    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert!(
        s.drag_started(),
        "the cursor and the guards still see a bar drag"
    );
    assert_eq!(s.drag_note(), None, "nothing is said before the release");
    assert_eq!(s.link_refusal(), None, "a finished task is no refusal");
    s.pointer_up();

    assert_eq!(s.drag_note(), Some(NOTE));
    assert_eq!(
        s.link_refusal(),
        None,
        "the note is not the red line's refusal"
    );
    let status = s.status_json(&RenderFacts::default());
    assert!(
        status.contains(r#""note":"Space planning is finished"#),
        "{status}"
    );
    assert_eq!(s.schedule().tasks(), tasks);
    assert_eq!(*s.solution(), solution);
    assert_eq!((s.revision(), s.stamp()), (revision, stamp));

    s.pointer_down(x, y, false);
    assert_eq!(s.drag_note(), None, "the next press clears it");
    s.pointer_up();
}

#[test]
fn a_click_on_a_finished_task_says_nothing() {
    let (mut s, (x, y)) = finished_session();
    s.pointer_down(x, y, false);
    s.pointer_up();
    assert_eq!(s.link_refusal(), None);
    assert_eq!(s.drag_note(), None);
}

/// A task with an actual start and no actual finish does not drag (gs#703): the engine refuses the
/// move, and the drag says so in the calm note with the task's name, not in the red refusal line.
#[test]
fn a_drag_of_an_in_progress_task_says_it_has_started_in_the_note_and_not_the_red_line() {
    let (mut s, (x, y)) = session_with(None);
    let tasks = s.schedule().tasks().to_vec();
    let solution = s.solution().clone();
    let note = "Space planning has started: its start is its actual start. Change Remaining or % complete in the table to move its finish.";

    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert_eq!(s.drag_note(), Some(note));
    assert_eq!(s.link_refusal(), None, "the red line stays empty");
    s.pointer_up();

    assert_eq!(s.drag_note(), Some(note), "the release keeps it");
    assert_eq!(s.link_refusal(), None);
    assert_eq!(s.schedule().tasks(), tasks);
    assert_eq!(*s.solution(), solution);

    s.pointer_down(x, y, false);
    assert_eq!(s.drag_note(), None, "the next press clears it");
    s.pointer_up();
}
