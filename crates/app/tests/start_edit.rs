//! Typing the Start cell (gs#703): the text becomes a start-no-earlier-than through `Edit::Move`,
//! as one undo step. A chain of three tasks of eight working hours each on a Monday-to-Friday
//! calendar with 08:00-12:00 and 13:00-17:00 shifts; day 0 is the origin Monday 2026-01-05.

use crate::common;

use app::{
    Session, Texts,
    edit::{
        Edit,
        start::{StartError, StartOutcome},
    },
    meta::TaskMeta,
};
use common::{clock, collapse::standard_calendar};
use engine::{Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;
const FIRST: TaskId = TaskId(0);
const SECOND: TaskId = TaskId(1);

pub fn chain() -> Schedule {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let ids: Vec<TaskId> = (0..3)
        .map(|_| {
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
                .expect("the calendar belongs to the schedule")
        })
        .collect();
    for pair in ids.windows(2) {
        schedule
            .add_link(Dependency {
                pred: pair[0],
                succ: pair[1],
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("a chain has no loop");
    }
    schedule
}

pub fn session() -> Session {
    let mut schedule = chain();
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 3],
        clock,
    )
}

#[test]
fn a_typed_start_is_a_start_no_earlier_than() {
    let mut s = session();
    let before = s.solution().clone();
    assert_eq!(s.set_start_text(FIRST, "2026-01-12"), Ok(StartOutcome::Set));
    assert_eq!(s.schedule().tasks()[0].snet, Some(7 * DAY));
    let mut fresh = chain();
    fresh.set_snet(FIRST, Some(7 * DAY)).unwrap();
    assert_eq!(
        *s.solution(),
        fresh.compute().unwrap(),
        "same as a full compute"
    );
    assert!(
        s.solution().dates[0].es >= 7 * DAY,
        "the task moved to the date"
    );
    assert_ne!(*s.solution(), before);
    // A time of day is taken too.
    assert_eq!(
        s.set_start_text(FIRST, "2026-01-12 10:00"),
        Ok(StartOutcome::Set)
    );
    assert_eq!(s.schedule().tasks()[0].snet, Some(7 * DAY + 10 * HOUR));
}

#[test]
fn a_started_task_refuses_a_typed_start_and_says_why() {
    let mut s = session();
    s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(8 * HOUR),
        actual_finish: None,
        remaining: Some(8 * HOUR),
    })
    .unwrap();
    let stamp = s.stamp();
    let error = s.set_start_text(FIRST, "2026-01-12").unwrap_err();
    assert_eq!(error, StartError::Edit(app::edit::EditError::HasActuals));
    assert!(error.to_string().contains("actual start or finish"));
    assert_eq!(s.schedule().tasks()[0].snet, None);
    assert_eq!(s.stamp(), stamp, "nothing changed");
}

#[test]
fn one_undo_restores_the_constraint() {
    let mut s = session();
    let before = s.solution().clone();
    s.set_start_text(SECOND, "2026-01-19").unwrap();
    assert_eq!(s.schedule().tasks()[1].snet, Some(14 * DAY));
    assert!(s.undo().unwrap());
    assert_eq!(s.schedule().tasks()[1].snet, None);
    assert_eq!(*s.solution(), before);
    assert!(s.redo().unwrap());
    assert_eq!(s.schedule().tasks()[1].snet, Some(14 * DAY));
}

#[test]
fn empty_text_removes_the_constraint_as_one_undo_step() {
    let mut s = session();
    s.set_start_text(SECOND, "2026-01-19").unwrap();
    assert_eq!(s.set_start_text(SECOND, ""), Ok(StartOutcome::Set));
    assert_eq!(s.schedule().tasks()[1].snet, None);
    assert!(s.undo().unwrap());
    assert_eq!(s.schedule().tasks()[1].snet, Some(14 * DAY));
}

#[test]
fn a_date_the_links_already_pass_changes_nothing_and_says_so() {
    let mut s = session();
    let stamp = s.stamp();
    let outcome = s.set_start_text(SECOND, "2026-01-05").unwrap();
    assert_eq!(outcome, StartOutcome::AlreadyLater);
    assert!(
        outcome
            .note(&Texts::default())
            .unwrap()
            .contains("nothing changed")
    );
    assert_eq!(s.schedule().tasks()[1].snet, None);
    assert_eq!(s.stamp(), stamp);
    assert!(!s.undo().unwrap(), "no history entry");
}

#[test]
fn text_that_is_not_a_date_is_refused() {
    let mut s = session();
    for text in ["soon", "2026-13-01", "2026-01-12 25:00", "2026-01-12 9:00"] {
        assert_eq!(
            s.set_start_text(FIRST, text),
            Err(StartError::BadDate),
            "{text}"
        );
    }
    assert_eq!(s.schedule().tasks()[0].snet, None);
}

#[test]
fn a_summary_refuses_a_typed_start() {
    let s = &mut common::collapse::session(30);
    let summary = common::collapse::summaries(s)[0];
    let error = s.set_start_text(summary, "2026-01-12").unwrap_err();
    assert!(matches!(error, StartError::Summary { .. }), "{error}");
    assert!(error.to_string().contains("summary"));
}

/// The chain as an import leaves it: every task floored at the origin Monday (gs#703 K3).
fn floored() -> Session {
    let mut schedule = chain();
    (0..3).for_each(|i| schedule.set_snet(TaskId(i), Some(0)).expect("a task"));
    let solution = schedule.compute().expect("computes");
    let meta = TaskMeta {
        floor: Some(0),
        ..TaskMeta::default()
    };
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, vec![meta; 3], clock)
}

#[test]
fn clearing_a_typed_start_puts_the_task_back_at_its_floor() {
    let mut s = floored();
    s.set_start_text(SECOND, "2026-01-19").unwrap();
    assert_eq!(s.start_constraint(SECOND), Some(14 * DAY));
    assert_eq!(s.set_start_text(SECOND, ""), Ok(StartOutcome::Set));
    assert_eq!(s.schedule().tasks()[1].snet, Some(0), "the floor stays");
    assert_eq!(s.start_constraint(SECOND), None);
    assert!(s.undo().unwrap());
    assert_eq!(s.schedule().tasks()[1].snet, Some(14 * DAY));
}

#[test]
fn a_task_with_only_its_floor_has_nothing_to_remove() {
    let mut s = floored();
    let stamp = s.stamp();
    let outcome = s.set_start_text(SECOND, "").unwrap();
    assert_eq!(outcome, StartOutcome::NothingToRemove);
    assert_eq!(
        outcome.note(&Texts::default()).as_deref(),
        Some("This task has no start constraint to remove.")
    );
    assert_eq!((s.schedule().tasks()[1].snet, s.stamp()), (Some(0), stamp));
    assert!(!s.undo().unwrap(), "no history entry");
}

#[test]
fn a_commit_counts_a_floor_as_no_start() {
    let mut s = floored();
    s.set_start_text(SECOND, "2026-01-19").unwrap();
    s.set_start_text(SECOND, "").unwrap();
    let json = s.take_commits();
    let (typed, cleared) = (
        r#""old":null,"new":"2026-01-19T00:00""#,
        r#""old":"2026-01-19T00:00","new":null"#,
    );
    assert!(json.contains(typed) && json.contains(cleared), "{json}");
}
