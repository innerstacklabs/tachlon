//! Typing a percent complete (gs#703): the remaining duration rounds to the nearest whole working hour, and a
//! space before the `%` is accepted. One task of eight working hours (480 minutes) unless stated on a
//! Monday-to-Friday calendar with 08:00-12:00 and 13:00-17:00 shifts.

use crate::common;

use app::{Session, edit::EditError, meta::TaskMeta};
use common::{clock, collapse::standard_calendar};
use engine::{Instant, Schedule, Task, TaskId};
use timeline::Timeline;

const FIRST: TaskId = TaskId(0);

fn session() -> Session {
    session_of(8)
}

fn session_of(hours: i64) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    schedule
        .add_task(Task {
            duration: hours * 60,
            calendar,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("the calendar belongs to the schedule");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(vec![0], 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 1],
        clock,
    )
}

fn fields(s: &Session) -> (Option<Instant>, Option<Instant>, Option<i64>) {
    let t = &s.schedule().tasks()[0];
    (t.actual_start, t.actual_finish, t.remaining)
}

#[test]
fn a_typed_percent_rounds_the_remaining_to_the_nearest_whole_hour() {
    let mut s = session();
    s.set_percent_text(FIRST, "33").expect("33 is a percent");
    // 8 hours x 67 / 100 = 5.36 hours: nearest hour is 5 (300 minutes), not 322 minutes.
    assert_eq!(fields(&s).2, Some(300));
}

#[test]
fn a_space_before_the_sign_is_accepted_and_means_the_same() {
    let mut spaced = session();
    spaced.set_percent_text(FIRST, "75 %").expect("accepted");
    let mut tight = session();
    tight.set_percent_text(FIRST, "75%").expect("accepted");
    assert_eq!(fields(&spaced), fields(&tight));
    assert_eq!(fields(&spaced).2, Some(120));
}

#[test]
fn a_space_between_digits_is_still_refused() {
    let mut s = session();
    let before = fields(&s);
    assert_eq!(s.set_percent_text(FIRST, "7 5"), Err(EditError::BadPercent));
    assert_eq!(fields(&s), before);
}

#[test]
fn a_remainder_of_exactly_half_an_hour_rounds_up() {
    let mut s = session_of(6);
    s.set_percent_text(FIRST, "25").expect("25 is a percent");
    // 6 hours x 75 / 100 = 4.5 hours: halves up makes it 5 (300 minutes), not 270.
    assert_eq!(fields(&s).2, Some(300));
}
