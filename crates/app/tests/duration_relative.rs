//! A Dur cell takes `+3d` and `-1d` as a change to the task's duration (gantt#703, S1); a result
//! below zero or a bare `+` is refused and changes nothing; `3d` still means 3d.

use crate::common::demo_session;

use app::{
    Session,
    edit::{DurationError, Edit, day_minutes, format_duration, parse_duration_cell},
};
use engine::TaskId;

const TASK: TaskId = TaskId(4);

fn set(session: &mut Session, minutes: i64) {
    let edit = Edit::SetDuration {
        task: TASK,
        minutes,
    };
    session.edit(edit).expect("applies");
}

fn type_in(text: &str) -> (Result<String, DurationError>, Session) {
    let (mut session, _) = demo_session(12);
    let day = day_minutes(session.schedule().calendar_of(TASK));
    set(&mut session, 5 * day);
    let shown = parse_duration_cell(text, session.schedule(), TASK).map(|minutes| {
        set(&mut session, minutes);
        format_duration(minutes, day)
    });
    (shown, session)
}

#[test]
fn plus_and_minus_change_the_duration_and_a_plain_duration_sets_it() {
    for (text, shown) in [("+3d", "8d"), ("-1d", "4d"), ("+4h", "5d 4h"), ("3d", "3d")] {
        assert_eq!(type_in(text).0.as_deref(), Ok(shown), "{text}");
    }
    let (note, typed) = (type_in("+3d").1, type_in("8d").1);
    assert!(note.move_report().is_some());
    assert_eq!(note.move_report(), typed.move_report());
}

#[test]
fn a_result_below_zero_or_a_bare_sign_is_refused_and_changes_nothing() {
    let duration = |s: &Session| s.schedule().tasks()[TASK.0 as usize].duration;
    let untouched = duration(&type_in("5d").1);
    for (text, error) in [
        ("-6d", DurationError::Negative),
        ("+", DurationError::Empty),
    ] {
        let (shown, session) = type_in(text);
        assert_eq!(shown, Err(error), "{text}");
        assert_eq!(duration(&session), untouched, "{text}");
    }
}
