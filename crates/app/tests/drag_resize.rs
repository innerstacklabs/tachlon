//! Dragging a bar's right edge (gantt#1875): the duration becomes the working time up to the end
//! of the day under the pointer, and one undo puts it back. A finished task, a milestone and a
//! summary keep the behavior they had.

use crate::common;

use app::{Session, edit::Edit, meta::TaskMeta};
use common::{
    clock,
    collapse::{schedule_with_tasks, summaries},
    outline_session,
};
use engine::{Dependency, LinkType, Solution, Task, TaskId};
use timeline::Timeline;

const DAY: i64 = 24 * 60;
const HOUR: i64 = 60;
/// Thursday noon: three days on from task 0's finish.
const THURSDAY: i64 = 3 * DAY + 12 * HOUR;
/// Task 0's duration after a frame at [`THURSDAY`]: Monday to Thursday.
const STRETCHED: i64 = 32 * HOUR;

/// Tasks 0 -> 1 are one working day each from Monday 08:00; task 3 has started, task 4 is
/// finished, task 5 is a milestone. One pixel is ten minutes.
fn session() -> Session {
    let mut schedule = schedule_with_tasks(6);
    for t in 0..5 {
        schedule
            .set_duration(TaskId(t), 8 * HOUR)
            .expect("a duration");
    }
    let (pred, succ) = (TaskId(0), TaskId(1));
    let link = Dependency {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    };
    schedule.add_dependency(link).expect("a link");
    let solution = schedule.compute().expect("computes");
    let mut timeline = Timeline::new((0..6).collect(), 1_000.0, 400.0, 1.0);
    timeline.viewport.px_per_minute = 0.1;
    timeline.viewport.t0 = 0.0;
    let meta = vec![TaskMeta::default(); 6];
    let mut s = Session::new(schedule, solution, timeline, meta, clock);
    for (t, actual_finish) in [(3, None), (4, Some(17 * HOUR))] {
        let (task, remaining) = (TaskId(t), actual_finish.is_none().then_some(8 * HOUR));
        let actual_start = Some(8 * HOUR);
        let progress = Edit::SetProgress {
            task,
            actual_start,
            actual_finish,
            remaining,
        };
        s.edit(progress).expect("applies");
    }
    s.with_history_cap(100) // the progress edits are setup, not history
}

/// The tasks and the solution.
fn state(s: &Session) -> (Vec<Task>, Solution) {
    (s.schedule().tasks().to_vec(), s.solution().clone())
}

fn duration(s: &Session) -> i64 {
    s.schedule().tasks()[0].duration
}

/// The canvas point `px` pixels left of the end of task `task`'s bar, on its row.
fn point(s: &Session, task: u32, px: f64) -> (f64, f64) {
    let (v, ef) = (&s.timeline().viewport, s.solution().dates[task as usize].ef);
    let row = s.timeline().row_of_task(task).expect("a row");
    (v.x_of(ef as f64) - px, v.row_top(row) + v.row_height / 2.0)
}

/// Move the pointer to instant `to` on row height `y` and take a frame.
fn frame(s: &mut Session, y: f64, to: i64) {
    s.pointer_move(s.timeline().viewport.x_of(to as f64), y);
    s.apply_drag();
}

/// Press task `task`'s edge and take one frame at instant `to`. Returns the row's `y`.
fn drag(s: &mut Session, task: u32, to: i64) -> f64 {
    let (x, y) = point(s, task, 1.0);
    s.pointer_down(x, y, false);
    frame(s, y, to);
    y
}

#[test]
fn a_drag_of_the_edge_sets_the_duration_and_one_undo_or_a_cancel_puts_it_back() {
    let mut s = session();
    let before = state(&s);
    drag(&mut s, 0, THURSDAY);
    assert_eq!(s.gesture().name(), "resize-dragging");
    assert_eq!(duration(&s), STRETCHED);
    assert_eq!(s.schedule().tasks()[0].snet, None, "the start is not asked");
    assert_ne!(
        s.solution().dates[1],
        before.1.dates[1],
        "the successor follows"
    );
    assert_eq!(
        s.solution(),
        &s.schedule().clone().compute().expect("computes")
    );
    s.pointer_up();
    assert_eq!(s.history_len(), (1, 0), "one step");
    assert!(s.undo().expect("undoes"));
    assert_eq!(state(&s), before);
    // A cancel puts it back too, and records nothing.
    let revision = s.revision();
    drag(&mut s, 0, THURSDAY);
    assert_eq!(duration(&s), STRETCHED);
    s.pointer_cancel();
    assert_eq!(state(&s), before);
    assert_eq!((s.history_len(), s.revision()), ((0, 1), revision));
}

/// A release on the press day, or after a frame that went to another day and came back, records
/// nothing and keeps the revision.
#[test]
fn a_release_on_the_pressed_day_records_nothing_and_keeps_the_revision() {
    let mut s = session();
    let (before, revision) = (state(&s), s.revision());
    for out_and_back in [false, true] {
        let y = drag(&mut s, 0, if out_and_back { THURSDAY } else { DAY - 1 });
        if out_and_back {
            assert_eq!(duration(&s), STRETCHED);
            frame(&mut s, y, DAY - 1); // the press day's last minute
        }
        assert_eq!(s.gesture().name(), "resize-dragging");
        s.pointer_up();
        assert_eq!(state(&s), before);
        assert_eq!((s.history_len(), s.revision()), ((0, 0), revision));
    }
}

#[test]
fn a_started_task_keeps_its_duration_and_the_drag_says_why() {
    let mut s = session();
    let (before, revision, stamp) = (state(&s), s.revision(), s.stamp());
    drag(&mut s, 3, THURSDAY);
    s.pointer_up();
    let note = " has started: change Remaining or % complete in the table to move its finish.";
    assert_eq!(s.drag_note(), Some(note));
    assert_eq!(state(&s), before);
    assert_eq!(
        (s.revision(), s.stamp(), s.history_len()),
        (revision, stamp, (0, 0))
    );
}

/// A finished task's edge press is the finished press, a milestone has no edge (its press moves
/// its start), and a summary's press pans: each does what it did before.
#[test]
fn presses_on_a_finished_task_a_milestone_and_a_summary_are_no_resize() {
    let mut s = session();
    let (x, y) = point(&s, 4, 1.0);
    s.pointer_down(x, y, false);
    assert_eq!(s.gesture().name(), "finished-pressed");
    s.pointer_up();
    let (x, y) = point(&s, 5, 0.0);
    s.pointer_down(x, y, false);
    assert_eq!(s.gesture().name(), "bar-pressed");
    frame(&mut s, y, 3 * DAY);
    s.pointer_up();
    assert!(s.schedule().tasks()[5].snet.is_some(), "the drag moved it");

    let (mut s, _) = outline_session(40);
    let summary = summaries(&s)[0].0;
    let (_, end) = s.timeline().summaries().get(summary).expect("a span");
    let v = &s.timeline().viewport;
    let row = s.timeline().row_of_task(summary).expect("a row");
    let (x, y) = (
        v.x_of(end as f64) - 1.0,
        v.row_top(row) + v.row_height / 2.0,
    );
    s.pointer_down(x, y, false);
    assert_eq!(s.gesture().name(), "idle");
}
