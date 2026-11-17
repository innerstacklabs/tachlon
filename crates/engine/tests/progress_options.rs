//! The two progress options (decision 0018): `unstartedRemaining` changes what is solved,
//! `startedLateStart` changes only what is shown. Calendar: Monday to Friday, 08:00 to 17:00; day 0
//! is the origin Monday.

mod common;

use common::equiv::assert_duration_cone;
use common::{at, dep, std_cal, task};
use engine::LinkType::FS;
use engine::{
    CalendarId, DEFAULT_STARTED_LATE_START, DEFAULT_UNSTARTED_REMAINING, Limit, Schedule,
    ScheduleOptions, StartedLateStart, TaskId, UnstartedRemaining,
};

fn days(n: i64) -> i64 {
    n * 9 * 60
}

fn options(started: StartedLateStart, unstarted: UnstartedRemaining) -> ScheduleOptions {
    ScheduleOptions {
        started_late_start: started,
        unstarted_remaining: unstarted,
    }
}

fn calendar(s: &mut Schedule) -> CalendarId {
    s.add_calendar(std_cal(&[]))
}

/// One unstarted task of five days with three remaining, and a one-day successor.
fn unstarted(on: UnstartedRemaining) -> (Schedule, TaskId, TaskId) {
    let mut s = Schedule::new();
    s.set_options(options(DEFAULT_STARTED_LATE_START, on));
    let c = calendar(&mut s);
    let long = task(&mut s, c, days(5), None);
    s.set_remaining(long, Some(days(3))).expect("exists");
    let after = task(&mut s, c, days(1), None);
    dep(&mut s, long, after, FS, 0);
    (s, long, after)
}

/// By hand: off, the five days run Monday to Friday and the successor starts Monday of the next
/// week (day 7); on, the three remaining days run Monday to Wednesday and the successor starts
/// Thursday (day 3).
#[test]
fn an_unstarted_remaining_sets_the_length_only_with_the_option_on() {
    let (mut off, long, after) = unstarted(UnstartedRemaining::Off);
    let sol = off.compute().expect("computes");
    assert_eq!(sol.get(long).ef, at(4, 17, 0));
    assert_eq!(sol.get(after).es, at(7, 8, 0));
    let (mut on, long, after) = unstarted(UnstartedRemaining::On);
    let sol = on.compute().expect("computes");
    assert_eq!(sol.get(long).ef, at(2, 17, 0));
    assert_eq!(sol.get(after).es, at(3, 8, 0));
    assert_eq!(sol.project_finish, at(3, 17, 0));
}

#[test]
fn a_remaining_of_zero_makes_an_unstarted_task_a_milestone_with_the_option_on() {
    let (mut s, long, after) = unstarted(UnstartedRemaining::On);
    s.set_remaining(long, Some(0)).expect("exists");
    let sol = s.compute().expect("computes");
    assert_eq!(sol.get(long).es, sol.get(long).ef);
    assert_eq!(sol.get(after).es, at(0, 8, 0));
}

#[test]
fn the_duration_cone_and_the_full_solve_agree_with_the_option_on() {
    let (mut s, long, after) = unstarted(UnstartedRemaining::On);
    let mut sol = s.compute().expect("computes");
    // The task's remaining sets its length, so a longer duration moves nothing.
    assert_duration_cone(&mut s, &mut sol, long, days(7), "long");
    assert_eq!(sol.get(long).ef, at(2, 17, 0));
    assert_duration_cone(&mut s, &mut sol, after, days(2), "after");
    assert_eq!(sol.get(after).ef, at(4, 17, 0));
    // With no remaining the whole duration is the length again, and the cone follows it.
    s.set_remaining(long, None).expect("exists");
    let mut sol = s.compute().expect("computes");
    assert_duration_cone(&mut s, &mut sol, long, days(7), "whole");
    assert_eq!(sol.get(long).ef, at(8, 17, 0));
}

#[test]
fn a_started_task_is_unchanged_by_the_first_option() {
    let build = |on| {
        let (mut s, long, _) = unstarted(on);
        s.set_actual_start(long, Some(at(0, 8, 0))).expect("exists");
        s.compute().expect("computes")
    };
    assert_eq!(
        build(UnstartedRemaining::Off),
        build(UnstartedRemaining::On)
    );
}

#[test]
fn a_new_schedule_has_the_default_constants() {
    assert_eq!(
        Schedule::new().options(),
        options(DEFAULT_STARTED_LATE_START, DEFAULT_UNSTARTED_REMAINING)
    );
}

/// Solve `s` with each value of the second option and check that nothing solved differs. Returns
/// the late start shown for `task` under `p6` and under `project`.
fn shown_both_ways(s: &mut Schedule, task: TaskId) -> (i64, i64) {
    let mut shown = Vec::new();
    let mut solutions = Vec::new();
    for value in [StartedLateStart::P6, StartedLateStart::Project] {
        s.set_options(options(value, DEFAULT_UNSTARTED_REMAINING));
        let sol = s.compute().expect("computes");
        shown.push(s.late_start_shown(task, sol.get(task)));
        solutions.push(sol);
    }
    assert_eq!(solutions[0], solutions[1], "float, dates and criticality");
    (shown[0], shown[1])
}

/// A started five-day task, two days left, behind plan with the data date on Wednesday: its remaining
/// work runs Wednesday and Thursday and its late start is Friday (day 4), but it started Monday. An
/// unstarted task waits for the data date, and shows its own late start.
#[test]
fn a_started_task_behind_plan_shows_its_actual_start() {
    let mut s = Schedule::new();
    let c = calendar(&mut s);
    let behind = task(&mut s, c, days(5), None);
    s.set_actual_start(behind, Some(at(0, 8, 0)))
        .expect("exists");
    s.set_remaining(behind, Some(days(2))).expect("exists");
    let next = task(&mut s, c, days(1), None);
    let waiting = task(&mut s, c, days(5), None);
    dep(&mut s, behind, next, FS, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    assert_eq!(shown_both_ways(&mut s, behind), (at(4, 8, 0), at(0, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!(sol.get(waiting).es, at(2, 8, 0));
    assert_eq!(
        s.late_start_shown(waiting, sol.get(waiting)),
        sol.get(waiting).ls
    );
}

/// A task started on Tuesday while its predecessor runs to Friday: its remaining work waits for the
/// predecessor, so the late start of that work is on or after day 7, and the actual start is day 1.
#[test]
fn a_task_started_before_its_predecessor_finished_shows_its_actual_start() {
    let mut s = Schedule::new();
    let c = calendar(&mut s);
    let before = task(&mut s, c, days(5), None);
    let started = task(&mut s, c, days(3), None);
    s.set_actual_start(started, Some(at(1, 8, 0)))
        .expect("exists");
    s.set_remaining(started, Some(days(2))).expect("exists");
    dep(&mut s, before, started, FS, 0);
    s.set_data_date(Some(at(1, 8, 0)));
    let (p6, project) = shown_both_ways(&mut s, started);
    assert_eq!(project, at(1, 8, 0));
    assert!(p6 >= at(7, 8, 0), "the remaining work waits: {p6}");
}

/// A started task that misses its finish limit (negative float, still critical) and a finished
/// task: the started one shows its actual start, the finished one the date the solution holds.
#[test]
fn a_started_task_that_misses_its_limit_shows_its_actual_start_beside_a_finished_one() {
    let mut s = Schedule::new();
    let c = calendar(&mut s);
    let late = task(&mut s, c, days(4), None);
    s.set_actual_start(late, Some(at(0, 8, 0))).expect("exists");
    s.set_remaining(late, Some(days(3))).expect("exists");
    let limit = Limit {
        fnlt: Some(at(3, 17, 0)),
        ..Limit::default()
    };
    s.set_limit(late, limit).expect("exists");
    let done = task(&mut s, c, days(2), None);
    s.set_actual_start(done, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(done, Some(at(1, 17, 0)))
        .expect("exists");
    s.set_data_date(Some(at(2, 8, 0)));
    let (p6, project) = shown_both_ways(&mut s, late);
    assert_eq!(project, at(0, 8, 0));
    assert_ne!(p6, project);
    let sol = s.compute().expect("computes");
    assert!(sol.get(late).float < 0 && sol.get(late).is_critical());
    assert_eq!(s.late_start_shown(done, sol.get(done)), sol.get(done).ls);
}
