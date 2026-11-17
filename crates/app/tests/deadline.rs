//! The session keeps each task's MS Project deadline for display (gs#703): filled at open, moved by
//! no edit, undo or redo, and no date, float or link follows from it.

use crate::common;

use app::{Session, document::Extras, export::export_mspdi, load_bytes};
use common::clock;
use engine::TaskId;
use import::{ProjectSelection, parse_mspdi};
use timeline::Timeline;

/// Two tasks; the project starts Wednesday 2026-01-07, so the origin is Monday 2026-01-05.
fn project(deadline: &str) -> String {
    let task = |uid: u32, extra: &str| {
        format!(
            "<Task><UID>{uid}</UID><Name>T{uid}</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary>\
<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>{extra}</Task>"
        )
    };
    format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2026-01-07T08:00:00</StartDate>\
<Tasks>{}{}</Tasks></Project>",
        task(1, deadline),
        task(2, "")
    )
}

/// A session over the file as `App::create` builds it, deadlines included.
fn open(deadline: &str) -> Session {
    let loaded = load_bytes(
        project(deadline).as_bytes(),
        &ProjectSelection::Unspecified,
        &clock,
    )
    .expect("the file loads");
    let timeline = Timeline::new(loaded.order, 1200.0, 600.0, 1.0);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        clock,
    )
    .with_origin(loaded.origin)
    .with_deadlines(loaded.deadlines)
}

fn dates(s: &Session) -> Vec<(i64, i64)> {
    s.solution().dates.iter().map(|d| (d.es, d.ef)).collect()
}

#[test]
fn an_edit_moves_no_deadline_and_a_deadline_moves_no_date() {
    let mut with = open("<Deadline>2026-01-12T16:00:00</Deadline>");
    let mut without = open("");
    // Monday 2026-01-12 16:00 is 7 days and 16 hours after the origin.
    let kept = Some(7 * 1440 + 16 * 60);
    assert_eq!(with.deadline(TaskId(0)), kept);
    assert_eq!(with.deadline(TaskId(1)), None);
    assert_eq!(without.deadline(TaskId(0)), None);
    assert_eq!(dates(&with), dates(&without));

    // Move the first task past its deadline: the dates move as they do without one, the deadline stays.
    for s in [&mut with, &mut without] {
        s.set_start_text(TaskId(0), "2026-01-19")
            .expect("the start sets");
    }
    assert_eq!(dates(&with), dates(&without));
    assert_eq!(with.deadline(TaskId(0)), kept);
    assert!(with.undo().expect("undoes"));
    assert_eq!(with.deadline(TaskId(0)), kept);
    assert!(with.redo().expect("redoes"));
    assert_eq!(with.deadline(TaskId(0)), kept);
    assert_eq!(with.deadline(TaskId(1)), None);
}

/// Saving as MS Project writes the deadline the session kept, after an edit too, and no longer lists it
/// as left out; a task that had none writes none.
#[test]
fn saving_as_ms_project_writes_each_tasks_deadline_back() {
    let mut session = open("<Deadline>2026-01-12T16:00:00</Deadline>");
    session
        .set_start_text(TaskId(0), "2026-01-19")
        .expect("the start sets");
    let extras = Extras {
        name: "deadline",
        notes: &[],
        source: None,
    };
    let exported = export_mspdi(&session, &extras).expect("exports");
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    assert_eq!(
        text.matches("<Deadline>2026-01-12T16:00:00</Deadline>")
            .count(),
        1,
        "{text}"
    );
    assert_eq!(text.matches("<Deadline>").count(), 1, "{text}");
    let read = parse_mspdi(&text).expect("reads back");
    let at = |i: usize| read.tasks[i].deadline;
    assert!(at(0).is_some() && at(1).is_none(), "{:?}", read.tasks);
    assert!(
        !exported
            .left_out
            .iter()
            .any(|line| line.contains("deadline")),
        "{:?}",
        exported.left_out
    );
}

/// The scene draws a `crit` tick on a task that finishes after its deadline, and none on one that
/// meets it or has none.
#[test]
fn the_scene_marks_a_task_past_its_deadline() {
    let ticks = |deadline: &str| {
        let s = open(deadline);
        let crit = s.theme().crit;
        s.scene().rects.iter().filter(|r| r.color == crit).count()
    };
    assert_eq!(ticks("<Deadline>2026-01-07T12:00:00</Deadline>"), 1);
    assert_eq!(ticks("<Deadline>2026-01-12T16:00:00</Deadline>"), 0);
    assert_eq!(ticks(""), 0);
}
