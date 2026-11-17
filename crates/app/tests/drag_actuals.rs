//! A bar drag on a task with actuals (gs#782): the engine refuses the move, the schedule stays
//! as it was, and the drag says why in the calm note (gs#703), not the red refusal line.

use crate::common;

use app::{
    RenderFacts, Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;

fn at(day: i64, hour: i64) -> Instant {
    day * DAY + hour * HOUR
}

/// Two unlinked tasks; the first has started. Returns the session and the canvas point on the
/// first task's bar (one pixel is ten minutes).
pub(crate) fn started_session() -> (Session, (f64, f64)) {
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
        actual_finish: None,
        remaining: Some(8 * HOUR),
    })
    .expect("applies");
    assert_eq!(
        s.timeline().hit_test(&s.solution().dates, x, y),
        Some(0),
        "the press must land on the bar"
    );
    (s, (x, y))
}

const NOTE: &str = "Space planning has started: its start is its actual start. Change Remaining or % complete in the table to move its finish.";

/// The engine still refuses the typed move; the drag's words are [`NOTE`].
fn typed_refusal(s: &mut Session) {
    let error = s
        .edit(Edit::Move {
            task: TaskId(0),
            snet: Some(at(3, 8)),
        })
        .expect_err("a started task does not drag");
    assert_eq!(error, EditError::HasActuals);
}

#[test]
fn a_drag_of_a_task_with_actuals_leaves_the_schedule_and_says_why() {
    let (mut s, (x, y)) = started_session();
    let tasks = s.schedule().tasks().to_vec();
    let solution = s.solution().clone();
    let (revision, stamp) = (s.revision(), s.stamp());
    typed_refusal(&mut s);

    s.pointer_down(x, y, false);
    assert_eq!(s.drag_note(), None, "a press says nothing yet");
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    assert_eq!(s.drag_note(), Some(NOTE), "the frame says why");
    assert_eq!(s.link_refusal(), None, "and not in the red line");
    let status = s.status_json(&RenderFacts::default());
    assert!(
        status.contains(r#""note":"Space planning has started"#),
        "{status}"
    );
    s.pointer_move(x + 500.0, y);
    s.apply_drag();
    s.pointer_up();

    assert_eq!(s.drag_note(), Some(NOTE), "it stays after the release");
    assert_eq!(s.schedule().tasks(), tasks);
    assert_eq!(*s.solution(), solution);
    assert_eq!((s.revision(), s.stamp()), (revision, stamp));
    assert!(s.drag_info().is_none(), "a refused frame records no drag");

    s.pointer_down(x, y, false);
    assert_eq!(s.drag_note(), None, "the next press clears it");
    s.pointer_up();
}

/// The note belongs to the press that met it: the next drag, on a task without actuals, clears
/// it and moves the task.
#[test]
fn a_drag_of_a_task_without_actuals_clears_the_refusal_and_moves() {
    let (mut s, (x, y)) = started_session();
    typed_refusal(&mut s);

    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    s.apply_drag();
    s.pointer_up();
    assert_eq!(s.drag_note(), Some(NOTE), "the started task said why");

    let row1 = s.timeline().viewport.row_top(1) + s.timeline().viewport.row_height / 2.0;
    s.pointer_down(x, row1, false);
    assert_eq!(s.drag_note(), None, "the press on the other task clears it");
    s.pointer_move(x + 300.0, row1);
    s.apply_drag();
    s.pointer_up();
    assert_eq!(s.drag_note(), None, "the unstarted task says nothing");
    assert!(s.schedule().tasks()[1].snet.is_some(), "the drag moved it");
}
