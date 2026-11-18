//! The resume date of a started task (gantt#1875): its remaining work picks up no earlier than
//! `Limit::resume`, as it does no earlier than the data date. Every date below was worked out by hand
//! (calendar: Monday to Friday, 08:00 to 17:00, nine working hours a day; day 0 is the origin Monday).

mod common;

use common::{at, dep, std_cal, task};
use engine::LinkType::FS;
use engine::{CalendarId, DurationCone, Limit, Schedule, TaskId};

/// Minutes in `n` working days of the test calendar.
fn days(n: i64) -> i64 {
    n * 9 * 60
}

/// Set the resume date of `t`.
fn resume(s: &mut Schedule, t: TaskId, at: i64) {
    let limit = Limit {
        resume: Some(at),
        ..Limit::default()
    };
    s.set_limit(t, limit).expect("task exists");
}

/// A started task: planned five days, started Monday 08:00, two days remaining.
fn started(s: &mut Schedule, c: CalendarId) -> TaskId {
    let a = task(s, c, days(5), None);
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_remaining(a, Some(days(2))).expect("exists");
    a
}

/// Resume on Wednesday 08:00: the two remaining days run Wednesday and Thursday, so the finish is
/// Thursday 17:00 (Tuesday 17:00 without it). The start stays at the actual start.
#[test]
fn a_started_task_resumes_no_earlier_than_its_resume_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c);
    resume(&mut s, a, at(2, 8, 0));
    let sol = s.compute().expect("computes");
    assert_eq!(sol.get(a).es, at(0, 8, 0));
    assert_eq!(sol.get(a).ef, at(3, 17, 0));
}

/// The data date is Wednesday 08:00 and the resume date Tuesday 08:00: the later one holds, so the
/// finish is Thursday 17:00 as it was before the resume date was set.
#[test]
fn a_resume_before_the_data_date_changes_nothing() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c);
    s.set_data_date(Some(at(2, 8, 0))).expect("in range");
    let before = s.compute().expect("computes");
    resume(&mut s, a, at(1, 8, 0));
    let sol = s.compute().expect("computes");
    assert_eq!(sol, before);
    assert_eq!(sol.get(a).ef, at(3, 17, 0));
}

/// An unstarted task ignores the resume date: one day from Monday 08:00, not from Wednesday.
#[test]
fn an_unstarted_task_ignores_the_resume_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(1), None);
    resume(&mut s, a, at(2, 8, 0));
    let sol = s.compute().expect("computes");
    assert_eq!(sol.get(a).es, at(0, 8, 0));
    assert_eq!(sol.get(a).ef, at(0, 17, 0));
}

/// A duration edit solved by the duration cone keeps the resume date. A (eight hours, started
/// Monday 08:00, so all eight hours remain) resumes Wednesday 08:00 and ends Wednesday 16:00;
/// shortened to six hours it ends Wednesday 14:00. A five-day task and thirty small ones let the
/// cone solve the edit itself.
#[test]
fn the_duration_cone_keeps_the_resume_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, 8 * 60, None);
    let b = task(&mut s, c, 8 * 60, None);
    let _long = task(&mut s, c, days(5), None);
    for _ in 0..30 {
        task(&mut s, c, 60, None);
    }
    dep(&mut s, a, b, FS, 0);
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    resume(&mut s, a, at(2, 8, 0));
    let mut sol = s.compute().expect("computes");
    assert_eq!(sol.get(a).ef, at(2, 16, 0));
    s.set_duration(a, 6 * 60).expect("task exists");
    let mut moved = Vec::new();
    let how = s
        .recompute_duration_cone_moved(&mut sol, a, &mut moved)
        .expect("solves");
    assert_eq!(how, DurationCone::Cone);
    assert_eq!(sol, s.compute().expect("computes"));
    assert_eq!(sol.get(a).ef, at(2, 14, 0));
}
