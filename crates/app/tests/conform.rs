//! A schedule opened in a chart whose host stores less is fitted to what the host stores (gantt#2090,
//! decision 0022, rule 8.3), and each kind of fit adds one note with its count.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::collapse::schedule_with_tasks;
use app::host::conform;
use engine::{DAY, Limit, Pin, TaskId};

type Fitted = (Vec<(i64, Limit)>, Vec<String>);

/// Three tasks on 8-hour days (480 minutes) with these durations and limits, fitted to `profile`: each task's duration
/// and limit afterwards, and the notes added.
fn fitted(lasts: &[i64], limits: &[Limit], profile: &str) -> Fitted {
    let mut schedule = schedule_with_tasks(3);
    for (id, (&last, &limit)) in (0..).map(TaskId).zip(lasts.iter().zip(limits)) {
        schedule.set_duration(id, last).unwrap();
        schedule.set_limit(id, limit).unwrap();
    }
    let mut notes = Vec::new();
    conform(&mut schedule, &[], &mut notes, profile).unwrap();
    let tasks = schedule.tasks().iter();
    (tasks.map(|t| (t.duration, t.limit)).collect(), notes)
}

#[test]
fn a_task_with_two_limits_keeps_the_first_and_no_profile_changes_nothing() {
    let d = Limit::default();
    let (snlt, fnlt) = (Some(3 * DAY), Some(4 * DAY));
    let (two, one) = (Limit { snlt, fnlt, ..d }, Limit { fnlt, ..d });
    let (tasks, notes) = fitted(&[480; 3], &[two, one, d], r#"{"perTask":1}"#);
    assert_eq!(tasks, [(480, Limit { snlt, ..d }), (480, one), (480, d)]);
    assert_eq!(
        notes,
        [
            "1 task had a second date limit, which this host does not store; the later one was dropped"
        ]
    );
    assert_eq!(
        fitted(&[90; 3], &[two; 3], "{}"),
        (vec![(90, two); 3], vec![])
    );
}

#[test]
fn a_duration_goes_to_the_nearest_whole_hour_and_never_to_none() {
    let d = [Limit::default(); 3];
    let (tasks, notes) = fitted(&[1, 90, 180], &d, r#"{"durationStep":"hours"}"#);
    assert_eq!(tasks, [(60, d[0]), (120, d[0]), (180, d[0])]);
    let said = "2 tasks had a duration that is not a whole number of hours, which this host does not store; each was rounded to the nearest";
    assert_eq!(notes, [said]);
}

#[test]
fn a_date_with_a_time_of_day_goes_to_the_start_of_its_day() {
    let d = Limit::default();
    let finish = Pin::Finish(DAY + 600);
    let whole = Limit {
        snlt: Some(DAY + 480),
        fnlt: Some(DAY + 1020),
        ..d
    };
    let limits = [
        Limit {
            snlt: Some(DAY + 600),
            ..d
        },
        Limit {
            pin: Some(finish),
            ..d
        },
        whole,
    ];
    let (tasks, notes) = fitted(&[480; 3], &limits, r#"{"dates":"day"}"#);
    let day = [
        Limit {
            snlt: Some(DAY),
            ..d
        },
        Limit {
            pin: Some(Pin::Finish(DAY)),
            ..d
        },
        whole,
    ];
    assert_eq!(tasks, [(480, day[0]), (480, day[1]), (480, whole)]);
    let said = "2 tasks had a date limit with a time of day, which this host does not store; each was moved to the start of its day";
    assert_eq!(notes, [said]);
}
