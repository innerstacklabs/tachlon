//! Recording a task's progress (gs#703, story 2): one edit sets the actual start, actual finish
//! and remaining duration, the schedule recomputes, and undo puts all three back. The dates are
//! worked out by hand: a chain of three tasks of eight working hours each on a Monday-to-Friday
//! calendar with 08:00-12:00 and 13:00-17:00 shifts; day 0 is the origin Monday, so Monday 08:00
//! is minute 480.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    Session,
    edit::{Edit, EditError},
    meta::TaskMeta,
};
use common::{clock, collapse::standard_calendar};
use engine::{
    Dependency, Instant, LinkType, MAX_DATE, RangeInput, RangeItem, Schedule, ScheduleError, Task,
    TaskId,
};
use timeline::Timeline;

const HOUR: i64 = 60;
const DAY: i64 = 24 * HOUR;

fn at(day: i64, hour: i64) -> Instant {
    day * DAY + hour * HOUR
}

/// Three tasks of eight working hours, each finish-to-start after the one before.
fn chain() -> Schedule {
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

fn session() -> Session {
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

const FIRST: TaskId = TaskId(0);

/// The first task started on Wednesday, with a day's work left.
fn started_wednesday() -> Edit {
    Edit::SetProgress {
        task: FIRST,
        actual_start: Some(at(2, 8)),
        actual_finish: None,
        remaining: Some(8 * HOUR),
    }
}

#[test]
fn a_progress_edit_matches_a_full_compute() {
    let mut s = session();
    let before = s.solution().clone();
    s.edit(started_wednesday()).expect("applies");
    let mut fresh = chain();
    fresh.set_actual_start(FIRST, Some(at(2, 8))).unwrap();
    fresh.set_remaining(FIRST, Some(8 * HOUR)).unwrap();
    let full = fresh.compute().expect("computes");
    assert_eq!(*s.solution(), full);
    assert_eq!(
        s.solution().dates[0].es,
        at(2, 8),
        "the actual start pins it"
    );
    assert_eq!(s.solution().dates[2].es, at(4, 8), "the chain follows");
    assert_ne!(*s.solution(), before);
    assert!(s.undo().unwrap());
    assert_eq!(*s.solution(), before, "undo puts every date back");
    assert!(s.redo().unwrap());
    assert_eq!(*s.solution(), full);
}

#[test]
fn undo_restores_the_fields_exactly_including_none() {
    let mut s = session();
    s.edit(started_wednesday()).expect("applies");
    s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(at(2, 8)),
        actual_finish: Some(at(2, 17)),
        remaining: Some(0),
    })
    .expect("a finish after the start applies");
    s.undo().unwrap();
    let task = &s.schedule().tasks()[0];
    assert_eq!(
        (task.actual_start, task.actual_finish, task.remaining),
        (Some(at(2, 8)), None, Some(8 * HOUR))
    );
    s.undo().unwrap();
    let task = &s.schedule().tasks()[0];
    assert_eq!(
        (task.actual_start, task.actual_finish, task.remaining),
        (None, None, None)
    );
}

#[test]
fn a_finish_before_the_start_is_refused_and_changes_nothing() {
    let mut s = session();
    let before = s.solution().clone();
    let error = s
        .edit(Edit::SetProgress {
            task: FIRST,
            actual_start: Some(at(2, 8)),
            actual_finish: Some(at(1, 8)),
            remaining: None,
        })
        .expect_err("a finish before the start");
    assert_eq!(error, EditError::FinishBeforeStart);
    assert_eq!(
        error.to_string(),
        "The actual finish is before the actual start."
    );
    assert_eq!(*s.solution(), before);
    assert_eq!(s.schedule().tasks()[0].actual_start, None);
    assert_eq!(s.history_len(), (0, 0));
}

#[test]
fn zero_remaining_on_a_started_task_is_allowed() {
    let mut s = session();
    s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(at(0, 8)),
        actual_finish: None,
        remaining: Some(0),
    })
    .expect("zero remaining applies");
    assert_eq!(s.schedule().tasks()[0].remaining, Some(0));
}

#[test]
fn a_negative_remaining_is_refused() {
    let mut s = session();
    let result = s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(at(0, 8)),
        actual_finish: None,
        remaining: Some(-1),
    });
    assert!(matches!(result, Err(EditError::Schedule(_))));
    assert_eq!(s.schedule().tasks()[0].actual_start, None);
}

#[test]
fn an_actual_start_past_the_range_is_refused_and_changes_nothing() {
    let mut s = session();
    s.edit(started_wednesday()).unwrap();
    let (before, history) = (fields(&s), s.history_len());
    let result = s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(MAX_DATE + 1),
        actual_finish: None,
        remaining: Some(4 * HOUR),
    });
    let want = ScheduleError::OutOfRange {
        input: RangeInput::ActualStart,
        item: RangeItem::Task(FIRST),
        value: MAX_DATE + 1,
    };
    assert_eq!(result, Err(EditError::Schedule(want)));
    assert_eq!((fields(&s), s.history_len()), (before, history));
}

#[test]
fn a_drag_of_a_task_with_actuals_is_refused() {
    let mut s = session();
    s.edit(started_wednesday()).expect("applies");
    let tasks = s.schedule().tasks().to_vec();
    let solution = s.solution().clone();
    let error = s
        .apply(Edit::Move {
            task: FIRST,
            snet: Some(at(3, 8)),
        })
        .expect_err("a started task does not drag");
    assert_eq!(error, EditError::HasActuals);
    assert!(error.to_string().contains("actual"), "{error}");
    assert_eq!(s.schedule().tasks(), tasks);
    assert_eq!(*s.solution(), solution);
    // A task without actuals still moves.
    s.apply(Edit::Move {
        task: TaskId(1),
        snet: Some(at(7, 8)),
    })
    .expect("an unstarted task drags");
}

#[test]
fn the_moved_report_names_the_tasks_a_progress_edit_moved() {
    let mut s = session();
    assert!(s.move_report().is_none());
    s.edit(started_wednesday()).expect("applies");
    let r = s.move_report().expect("the chain moved");
    let moved: Vec<TaskId> = r.was.iter().map(|m| m.task).collect();
    assert_eq!(moved, [TaskId(1), TaskId(2)], "{}", r.text);
    assert_eq!((r.others, r.finish_days), (2, 2), "{}", r.text);
    assert_eq!(r.text, "2 tasks moved · finish +2 days");
    assert!(r.edited.is_some(), "the edited task is reported apart");
}

/// The engine accepts an actual finish before the actual start (its setters store both and the
/// solver clamps the finish), so an import can hold them. A session over such dates.
fn reversed_session() -> Session {
    let mut schedule = chain();
    schedule.set_actual_start(FIRST, Some(at(2, 8))).unwrap();
    schedule.set_actual_finish(FIRST, Some(at(1, 8))).unwrap();
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 3],
        clock,
    )
}

fn fields(s: &Session) -> (Option<Instant>, Option<Instant>, Option<i64>) {
    let t = &s.schedule().tasks()[0];
    (t.actual_start, t.actual_finish, t.remaining)
}

fn clear() -> Edit {
    Edit::SetProgress {
        task: FIRST,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    }
}

#[test]
fn the_inverse_restores_dates_the_engine_accepted_in_reverse() {
    let mut s = reversed_session();
    let (fields_before, solution_before) = (fields(&s), s.solution().clone());
    assert_eq!(fields_before, (Some(at(2, 8)), Some(at(1, 8)), None));
    let inverse = s.apply(clear()).expect("clearing applies");
    assert_eq!(fields(&s), (None, None, None));
    let redo = s.apply(inverse).expect("the inverse applies");
    assert_eq!(fields(&s), fields_before);
    assert_eq!(*s.solution(), solution_before);
    assert_eq!(redo, clear());
}

#[test]
fn undo_and_redo_restore_dates_the_engine_accepted_in_reverse() {
    let mut s = reversed_session();
    let (fields_before, solution_before) = (fields(&s), s.solution().clone());
    s.edit(clear()).expect("clearing applies");
    let cleared = s.solution().clone();
    assert!(s.undo().expect("undo is not refused"));
    assert_eq!(fields(&s), fields_before);
    assert_eq!(*s.solution(), solution_before);
    assert!(s.redo().expect("redo is not refused"));
    assert_eq!(fields(&s), (None, None, None));
    assert_eq!(*s.solution(), cleared);
}

/// The cells show `YYYY-MM-DD HH:MM`, so the field setter reads the table's own text back.
#[test]
fn editing_one_progress_field_keeps_the_other_two_and_undo_restores_all_three() {
    let mut s = session();
    s.edit(started_wednesday()).expect("applies");
    let was = fields(&s);
    assert_eq!(was, (Some(at(2, 8)), None, Some(8 * HOUR)));

    s.set_progress_field_text(FIRST, "actual_finish", "2026-01-08")
        .expect("a finish after the start applies");
    assert_eq!(
        fields(&s),
        (Some(at(2, 8)), Some(at(3, 0)), Some(8 * HOUR)),
        "the start and the remaining stay"
    );
    s.set_progress_field_text(FIRST, "remaining", "4h")
        .expect("a remaining applies");
    assert_eq!(fields(&s), (Some(at(2, 8)), Some(at(3, 0)), Some(4 * HOUR)));
    s.set_progress_field_text(FIRST, "actual_start", "2026-01-07 09:30")
        .expect("the cell's own text applies");
    assert_eq!(
        fields(&s),
        (Some(at(2, 9) + 30), Some(at(3, 0)), Some(4 * HOUR))
    );
    assert_eq!(s.history_len().0, 4, "one undo step for each");

    for expected in [
        (Some(at(2, 8)), Some(at(3, 0)), Some(4 * HOUR)),
        (Some(at(2, 8)), Some(at(3, 0)), Some(8 * HOUR)),
        was,
    ] {
        assert!(s.undo().unwrap());
        assert_eq!(fields(&s), expected);
    }
    let mut fresh = chain();
    fresh.set_actual_start(FIRST, Some(at(2, 8))).unwrap();
    fresh.set_remaining(FIRST, Some(8 * HOUR)).unwrap();
    assert_eq!(*s.solution(), fresh.compute().expect("computes"));
}

#[test]
fn clearing_one_progress_field_leaves_the_others_and_a_bad_entry_changes_nothing() {
    let mut s = session();
    s.edit(started_wednesday()).expect("applies");
    s.set_progress_field_text(FIRST, "remaining", "  ")
        .expect("empty text clears the field");
    assert_eq!(fields(&s), (Some(at(2, 8)), None, None));

    let before = s.solution().clone();
    for (field, text) in [
        ("actual_start", "soon"),
        ("actual_start", "2026-01-07 25:00"),
        ("actual_finish", "2026-01-07"),
        ("remaining", "x"),
    ] {
        assert!(
            s.set_progress_field_text(FIRST, field, text).is_err(),
            "{field}: {text}"
        );
    }
    assert_eq!(fields(&s), (Some(at(2, 8)), None, None));
    assert_eq!(*s.solution(), before);
    let error = s
        .set_progress_field_text(FIRST, "actual_finish", "2026-01-07")
        .expect_err("a finish before the start");
    assert_eq!(error, EditError::FinishBeforeStart);
    assert_eq!(s.history_len().0, 2, "a refused entry is no undo step");
}

/// Data date Wednesday 00:00 (2026-01-07).
fn with_data_date() -> Session {
    session().with_data_date(Some(at(2, 0)))
}

#[test]
fn an_actual_after_the_data_date_is_refused_in_both_entry_paths_and_changes_nothing() {
    let mut s = with_data_date();
    let before = s.solution().clone();
    let cell = s
        .set_progress_field_text(FIRST, "actual_start", "2026-01-08")
        .expect_err("a start after the data date");
    assert_eq!(cell, EditError::ActualAfterDataDate);
    assert_eq!(
        cell.to_string(),
        "An actual date cannot be after the data date. Move the data date first, or write a date on or before it."
    );
    assert_eq!(
        s.set_progress_text(FIRST, "2026-01-06", "2026-01-08", ""),
        Err(EditError::ActualAfterDataDate),
        "the dialog's finish after the data date"
    );
    assert_eq!(fields(&s), (None, None, None));
    assert_eq!(*s.solution(), before);
    assert_eq!(s.history_len().0, 0, "a refused entry is no undo step");
    s.set_progress_field_text(FIRST, "actual_start", "2026-01-07")
        .expect("the data date itself is not after it");
}

#[test]
fn remaining_can_be_edited_on_a_task_an_import_left_after_the_data_date() {
    let mut s = with_data_date();
    s.edit(Edit::SetProgress {
        task: FIRST,
        actual_start: Some(at(3, 8)),
        actual_finish: None,
        remaining: Some(8 * HOUR),
    })
    .expect("an edit made without the typed check, as an import leaves it");
    s.set_progress_field_text(FIRST, "remaining", "4h")
        .expect("remaining is not an actual");
    s.set_progress_field_text(FIRST, "actual_start", "2026-01-08 08:00")
        .expect("the actual the task already holds");
    assert_eq!(fields(&s), (Some(at(3, 8)), None, Some(4 * HOUR)));
}
