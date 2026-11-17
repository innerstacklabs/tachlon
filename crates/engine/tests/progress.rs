//! Actual dates and remaining duration (gs#703, story 2): an actual start pins a bar, a finished
//! bar is fixed at its actuals, and a started bar finishes its remaining duration after the data
//! date (docs/decisions/0008-progress-model.md). Every expected date below was worked out by
//! hand (calendar: Monday to Friday, 08:00 to 17:00, nine working hours a day; day 0 is the
//! origin Monday).
//!
//! `check_invariants` is not used here: it asserts `ef == fin(es)` with the planned duration,
//! which a pinned bar does not satisfy by design.

mod common;

use common::{at, dep, std_cal, task};
use engine::LinkType::{FF, FS, SF, SS};
use engine::{Schedule, ScheduleError, TaskId};

/// Minutes in `n` working days of the test calendar.
fn days(n: i64) -> i64 {
    n * 9 * 60
}

/// A started task: planned `planned_days`, started at `start`, `remaining_days` left.
fn started(
    s: &mut Schedule,
    c: engine::CalendarId,
    planned_days: i64,
    start: i64,
    remaining_days: Option<i64>,
) -> TaskId {
    let t = task(s, c, days(planned_days), None);
    s.set_actual_start(t, Some(start)).expect("task exists");
    s.set_remaining(t, remaining_days.map(days))
        .expect("task exists");
    t
}

/// Five planned days, started Monday 08:00, two days remaining, data date Wednesday 08:00. The
/// start stays at the actual start; the two remaining days run from the data date, Wednesday and
/// Thursday, and end Thursday 17:00. The successor (one day, finish-to-start) begins when
/// Thursday ends: Friday 08:00 to 17:00.
#[test]
fn an_actual_start_pins_the_start_and_remaining_runs_from_the_data_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c, 5, at(0, 8, 0), Some(2));
    let b = task(&mut s, c, days(1), None);
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!(sol.get(a).es, at(0, 8, 0));
    assert_eq!(sol.get(a).ef, at(3, 17, 0));
    assert_eq!(sol.get(b).es, at(4, 8, 0));
    assert_eq!(sol.get(b).ef, at(4, 17, 0));
    assert_eq!(sol.project_finish, at(4, 17, 0));
}

/// A finished task is fixed at its actuals: started Monday 10:00, finished Tuesday 15:00, with a
/// data date of Thursday 08:00 that would otherwise move it. It stays, and a successor of one
/// day starts at the data date, not at the finished task's finish.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn a_finished_task_is_fixed_at_its_actuals() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(2), None);
    s.set_actual_start(a, Some(at(0, 10, 0))).expect("exists");
    s.set_actual_finish(a, Some(at(1, 15, 0))).expect("exists");
    let b = task(&mut s, c, days(1), None);
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(3, 8, 0)));
    let sol = s.compute().expect("computes");
    let d = sol.get(a);
    assert_eq!((d.es, d.ef), (at(0, 10, 0), at(1, 15, 0)));
    // A finished task has no float and cannot move later.
    assert_eq!((d.ls, d.lf, d.float), (at(0, 10, 0), at(1, 15, 0), 0));
    // B: the data date floors it, Thursday 08:00 to 17:00.
    assert_eq!(sol.get(b).es, at(3, 8, 0));
    assert_eq!(sol.get(b).ef, at(3, 17, 0));
}

/// A started task with no remaining duration set has its whole planned duration left. One planned
/// day, started Monday 08:00, data date Wednesday 08:00: the day runs Wednesday, 08:00 to 17:00.
#[test]
fn a_started_task_without_a_remaining_duration_has_all_its_duration_left() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c, 1, at(0, 8, 0), None);
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(0, 8, 0), at(2, 17, 0)));
}

/// Retained logic (decision 0008, pick 2): the remaining work of a started task waits for its
/// predecessors. P (two days, unstarted) runs from the data date, Wednesday 08:00, to Thursday
/// 17:00. A started Monday 08:00 with one day left, finish-to-start after P: its start stays
/// Monday, its remaining day runs Friday 08:00 to 17:00.
#[test]
fn the_remaining_work_of_a_started_task_waits_for_its_predecessors() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = task(&mut s, c, days(2), None);
    let a = started(&mut s, c, 3, at(0, 8, 0), Some(1));
    dep(&mut s, p, a, FS, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(p).es, sol.get(p).ef), (at(2, 8, 0), at(3, 17, 0)));
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(0, 8, 0), at(4, 17, 0)));
}

/// A predecessor that is not finished does not move a finished task: P is unstarted (Thursday
/// 08:00 to 17:00 from the data date), F is finished in the first week and follows P. F stays at
/// its actuals, and P, with nothing left to constrain it, is the project's critical task.
#[test]
fn a_finished_task_ignores_a_predecessor_that_has_not_finished() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = task(&mut s, c, days(1), None);
    let f = task(&mut s, c, days(1), None);
    s.set_actual_start(f, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(f, Some(at(0, 17, 0))).expect("exists");
    dep(&mut s, p, f, FS, 0);
    s.set_data_date(Some(at(3, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(f).es, sol.get(f).ef), (at(0, 8, 0), at(0, 17, 0)));
    assert_eq!((sol.get(p).es, sol.get(p).ef), (at(3, 8, 0), at(3, 17, 0)));
    assert_eq!(sol.get(p).float, 0);
}

/// A started task's own start-no-earlier-than no longer binds: the actual start is a fact.
/// SNET Wednesday, actual start Monday, one day left, data date Tuesday 08:00: the bar starts
/// Monday and the day runs Tuesday.
#[test]
fn an_actual_start_replaces_the_start_no_earlier_than() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(1), Some(at(2, 8, 0)));
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_data_date(Some(at(1, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(0, 8, 0), at(1, 17, 0)));
}

/// An actual start after the data date (a data-entry slip) still gives a bar that ends no earlier
/// than it starts: the remaining work cannot begin before the task did. Actual start Wednesday
/// 08:00, data date Monday 08:00, one day left: the day runs Wednesday.
#[test]
fn the_remaining_work_never_begins_before_the_actual_start() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c, 1, at(2, 8, 0), Some(1));
    s.set_data_date(Some(at(0, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(2, 8, 0), at(2, 17, 0)));
}

/// A task ahead of its plan does not break the backward pass. Ten planned days, started
/// Wednesday 08:00 of week 1 (day 2), two days left from a data date of Thursday 08:00 (day 3): it
/// ends Friday 17:00 (day 4), long before the planned ten days would. It is the whole project,
/// so the late dates come from the remaining work alone: late start Thursday 08:00 and late
/// finish Friday 17:00.
#[test]
fn a_task_ahead_of_its_plan_has_late_dates_from_its_remaining_work() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c, 10, at(2, 8, 0), Some(2));
    s.set_data_date(Some(at(3, 8, 0)));
    let sol = s.compute().expect("computes");
    let d = sol.get(a);
    assert_eq!((d.es, d.ef), (at(2, 8, 0), at(4, 17, 0)));
    assert_eq!((d.ls, d.lf), (at(3, 8, 0), at(4, 17, 0)));
}

/// An actual start on a Saturday (no work that day) is kept as entered; the schedule still
/// computes, and the remaining day runs from the data date's next working instant, Monday.
#[test]
fn an_actual_start_outside_working_time_is_kept() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    // Saturday of the origin week is day 5.
    let a = started(&mut s, c, 1, at(5, 10, 0), Some(1));
    s.set_data_date(Some(at(5, 12, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(5, 10, 0), at(7, 17, 0)));
}

/// Start-to-start and finish-to-finish links read a pinned bar's dates: B follows A start-to-start
/// with a lag of zero and finish-to-finish. A started Monday 08:00 with one day left from
/// Wednesday 08:00 (ends Wednesday 17:00); B (one day) cannot start before Monday 08:00 (A's start)
/// and cannot finish before Wednesday 17:00 (A's finish), and the data date is Wednesday 08:00, so B
/// runs Wednesday, 08:00 to 17:00.
#[test]
fn links_read_the_pinned_dates() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = started(&mut s, c, 3, at(0, 8, 0), Some(1));
    let b = task(&mut s, c, days(1), None);
    dep(&mut s, a, b, SS, 0);
    dep(&mut s, a, b, FF, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(0, 8, 0), at(2, 17, 0)));
    assert_eq!((sol.get(b).es, sol.get(b).ef), (at(2, 8, 0), at(2, 17, 0)));
}

/// A schedule with no actuals schedules as before: the three fields default to none and change
/// nothing, with or without a data date.
#[test]
fn clearing_the_actuals_restores_the_plan() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(2), None);
    let plan = s.compute().expect("computes");
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(a, Some(at(0, 17, 0))).expect("exists");
    s.set_remaining(a, Some(days(1))).expect("exists");
    assert_ne!(s.compute().expect("computes"), plan);
    s.set_actual_start(a, None).expect("exists");
    s.set_actual_finish(a, None).expect("exists");
    s.set_remaining(a, None).expect("exists");
    assert_eq!(s.compute().expect("computes"), plan);
}

/// A remaining duration below zero is refused and nothing changes; an unknown or deleted task is
/// refused by every setter.
#[test]
fn the_setters_refuse_what_they_cannot_keep() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(1), None);
    assert_eq!(
        s.set_remaining(a, Some(-1)),
        Err(ScheduleError::NegativeDuration(a))
    );
    assert_eq!(s.tasks()[0].remaining, None);
    let gone = TaskId(9);
    assert_eq!(
        s.set_actual_start(gone, Some(0)),
        Err(ScheduleError::UnknownTask(gone))
    );
    assert_eq!(
        s.set_actual_finish(gone, Some(0)),
        Err(ScheduleError::UnknownTask(gone))
    );
    assert_eq!(
        s.set_remaining(gone, Some(0)),
        Err(ScheduleError::UnknownTask(gone))
    );
    s.delete_task(a).expect("exists");
    assert_eq!(
        s.set_actual_start(a, Some(0)),
        Err(ScheduleError::UnknownTask(a))
    );
}

/// The cone of another task's constraint keeps working beside a pinned bar: dragging the
/// start-no-earlier-than of an unrelated predecessor moves a successor of the started task,
/// and the cone's answer equals a full compute.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn a_cone_beside_a_started_task_equals_a_full_compute() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = task(&mut s, c, days(1), None);
    let a = started(&mut s, c, 4, at(0, 8, 0), Some(2));
    let b = task(&mut s, c, days(1), None);
    let f = task(&mut s, c, days(1), None);
    s.set_actual_start(f, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(f, Some(at(0, 17, 0))).expect("exists");
    dep(&mut s, p, a, FS, 0);
    dep(&mut s, a, b, FS, 0);
    dep(&mut s, b, f, FS, 0);
    s.set_data_date(Some(at(1, 8, 0)));
    let mut sol = s.compute().expect("computes");
    s.set_snet(p, Some(at(3, 8, 0))).expect("exists");
    s.recompute_cone(&mut sol, p).expect("solves");
    assert_eq!(sol, s.compute().expect("computes"));
    assert_eq!(sol.get(a).ef, at(7, 17, 0));
}

/// A start-to-start successor of a started task bounds its fixed actual start, not its remaining
/// work. P: five planned days, started Monday 08:00, two days remaining, data date Wednesday
/// 08:00. Q (one day, unstarted) precedes P finish-to-start: Q runs Wednesday, P's remaining
/// days are Thursday and Friday (early finish Friday 17:00). S (ten days) follows P
/// start-to-start with a one-day lag: S starts at the data date, Wednesday 08:00, and finishes
/// Tuesday of the third week (day 15) 17:00, the project finish.
///
/// Late dates, by hand: S late start Wednesday 08:00 (day 2). S binds P's actual start (Monday),
/// which is met, and nothing later, so P's remaining two days may run as late as the project
/// finish: P late start Monday of week three (day 14) 08:00, late finish day 15 17:00. Q must end
/// before P's late start: Q late start and finish are Friday of week two (day 11), 08:00 and
/// 17:00. Q's early start (Wednesday, day 2) is before its late start. Before the fix, S's bound
/// capped P's late start at Tuesday and Q's late start fell before Q's early start.
#[test]
fn a_started_task_is_not_late_for_a_start_to_start_successor() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = started(&mut s, c, 5, at(0, 8, 0), Some(2));
    let q = task(&mut s, c, days(1), None);
    let succ = task(&mut s, c, days(10), None);
    dep(&mut s, q, p, FS, 0);
    dep(&mut s, p, succ, SS, days(1));
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    let (dp, dq, ds) = (sol.get(p), sol.get(q), sol.get(succ));
    assert_eq!((dq.es, dq.ef), (at(2, 8, 0), at(2, 17, 0)));
    assert_eq!((dp.es, dp.ef), (at(0, 8, 0), at(4, 17, 0)));
    assert_eq!((ds.es, ds.ef), (at(2, 8, 0), at(15, 17, 0)));
    assert_eq!(sol.project_finish, at(15, 17, 0));
    assert_eq!(ds.ls, at(2, 8, 0));
    assert_eq!((dp.ls, dp.lf), (at(14, 8, 0), at(15, 17, 0)));
    assert_eq!((dq.ls, dq.lf), (at(11, 8, 0), at(11, 17, 0)));
    assert!(dp.lf >= dp.ef);
    assert!(dq.es <= dq.ls);
}

/// The same project with a start-to-finish successor. P: five planned days, started Monday 08:00,
/// two days left, data date Wednesday 08:00. Q (one day) is a finish-to-start predecessor of P.
/// S (ten days) is a start-to-finish successor of P with a five-day lag: S may not finish before
/// P's start plus five working days (Monday of week two, 08:00).
///
/// Early dates, by hand: Q starts at the data date, Wednesday 08:00 to 17:00. P's remaining two
/// days wait for Q and run Thursday and Friday, so P keeps its start (Monday 08:00) and ends
/// Friday (day 4) 17:00. S starts at the data date, Wednesday 08:00, and its ten days end on
/// Tuesday of week three (day 15) 17:00, which meets the lag. The project finishes day 15 17:00.
///
/// Late dates, by hand: S late finish day 15 17:00, late start Wednesday (day 2). S binds P's
/// actual start, which is fixed and met, and nothing later, so P's remaining two days may run as
/// late as the project finish: P late start Monday of week three (day 14) 08:00, late finish
/// day 15 17:00. Q must end before P's late start: Friday of week two (day 11), 08:00 to 17:00.
/// Were S's bound applied to P (S's late finish less five days, about day 9), it would cap P's
/// late start and pull Q's late dates before Q's early start.
#[test]
fn a_started_task_is_not_late_for_a_start_to_finish_successor() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = started(&mut s, c, 5, at(0, 8, 0), Some(2));
    let q = task(&mut s, c, days(1), None);
    let succ = task(&mut s, c, days(10), None);
    dep(&mut s, q, p, FS, 0);
    dep(&mut s, p, succ, SF, days(5));
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    let (dp, dq, ds) = (sol.get(p), sol.get(q), sol.get(succ));
    assert_eq!((dq.es, dq.ef), (at(2, 8, 0), at(2, 17, 0)));
    assert_eq!((dp.es, dp.ef), (at(0, 8, 0), at(4, 17, 0)));
    assert_eq!((ds.es, ds.ef), (at(2, 8, 0), at(15, 17, 0)));
    assert_eq!(sol.project_finish, at(15, 17, 0));
    assert_eq!(ds.ls, at(2, 8, 0));
    assert_eq!((dp.ls, dp.lf), (at(14, 8, 0), at(15, 17, 0)));
    assert_eq!((dq.ls, dq.lf), (at(11, 8, 0), at(11, 17, 0)));
    assert!(dp.lf >= dp.ef);
    assert!(dq.es <= dq.ls);
}
