//! Start-no-later-than, finish-no-earlier-than and finish-no-later-than, worked by hand on a
//! Monday-to-Friday 08:00-17:00 calendar (540 working minutes a day). `brute_force.rs` and
//! `cone.rs` cover them on random schedules.

mod common;

use common::{at, dep, oracle, std_cal, task};
use engine::{
    CalendarId, DurationCone, FallbackReason, Instant, Limit, LinkType, Pin, Schedule, Solution,
    TaskId,
};

const D: i64 = 540;

fn week() -> (Schedule, CalendarId) {
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    (s, cal)
}

/// Set a task's start-no-later-than, finish-no-earlier-than and finish-no-later-than.
fn limit(s: &mut Schedule, t: TaskId, bounds: [Option<Instant>; 3]) {
    let limit = Limit {
        snlt: bounds[0],
        fnet: bounds[1],
        fnlt: bounds[2],
        ..Limit::default()
    };
    s.set_limit(t, limit).expect("the task exists");
}

/// `a` (one day) then `b` (one day), beside a five-day task that ends the project on Friday.
fn chain(s: &mut Schedule, cal: CalendarId) -> (TaskId, TaskId) {
    let (a, b) = (task(s, cal, D, None), task(s, cal, D, None));
    task(s, cal, 5 * D, None);
    dep(s, a, b, LinkType::FS, 0);
    (a, b)
}

/// Early start, early finish, late start, late finish, float.
type Dates = (i64, i64, i64, i64, i64);

fn dates(sol: &Solution, t: TaskId) -> Dates {
    let d = sol.get(t);
    (d.es, d.ef, d.ls, d.lf, d.float)
}

fn day(n: i64) -> (i64, i64) {
    (at(n, 8, 0), at(n, 17, 0))
}

#[test]
fn fnet_pushes_early_finish() {
    let (mut s, cal) = week();
    let t = task(&mut s, cal, D, None);
    limit(&mut s, t, [None, Some(day(2).1), None]);
    let sol = s.compute().unwrap();
    let (start, finish) = day(2);
    assert_eq!(dates(&sol, t), (start, finish, start, finish, 0));
}

#[test]
fn snlt_caps_the_late_start() {
    let (mut s, cal) = week();
    let (a, b) = chain(&mut s, cal);
    limit(&mut s, b, [Some(day(2).0), None, None]);
    let sol = s.compute().unwrap();
    let (d0, d1, d2) = (day(0), day(1), day(2));
    assert_eq!(dates(&sol, b), (d1.0, d1.1, d2.0, d2.1, D));
    assert_eq!(dates(&sol, a), (d0.0, d0.1, d1.0, d1.1, D));
    assert!(!sol.get(b).is_critical());
}

#[test]
fn fnlt_caps_the_late_finish() {
    let (mut s, cal) = week();
    let (_, b) = chain(&mut s, cal);
    limit(&mut s, b, [None, None, Some(day(3).1)]);
    let sol = s.compute().unwrap();
    let (d1, d3) = (day(1), day(3));
    assert_eq!(dates(&sol, b), (d1.0, d1.1, d3.0, d3.1, 2 * D));
}

/// A late limit before the early date is a negative float, on the task and on the predecessor
/// that feeds it, and both are critical; it is not an engine error.
#[test]
fn a_late_limit_that_cannot_be_met_gives_negative_float() {
    let (mut s, cal) = week();
    let (a, b) = chain(&mut s, cal);
    s.set_snet(a, Some(day(2).0)).unwrap();
    limit(&mut s, b, [Some(day(2).0), None, None]);
    let sol = s.compute().unwrap();
    let (d1, d2, d3) = (day(1), day(2), day(3));
    assert_eq!(dates(&sol, b), (d3.0, d3.1, d2.0, d2.1, -D));
    assert_eq!(dates(&sol, a), (d2.0, d2.1, d1.0, d1.1, -D));
    assert!(sol.get(a).is_critical() && sol.get(b).is_critical());
}

/// The dates of an `n`-day task that starts on working day `k` (0 is the project start) and is
/// `late` working days late; a late start can fall before the origin (gantt#2072).
fn late(k: i64, n: i64, late: i64) -> Dates {
    let wd = |k: i64| day(k.div_euclid(5) * 7 + k.rem_euclid(5));
    let [e, l] = [k, k - late].map(|k| (wd(k).0, wd(k + n - 1).1));
    (e.0, e.1, l.0, l.1, -late * D)
}

/// A finish-to-start path of tasks of these many days, with what `set` adds: each task reads
/// its hand-solved dates, and the brute-force oracle agrees.
fn path<const N: usize>(days: [i64; N], set: impl Fn(&mut Schedule, &[TaskId]), want: [Dates; N]) {
    let (mut s, cal) = week();
    let ids = days.map(|n| task(&mut s, cal, n * D, None));
    for w in ids.windows(2) {
        dep(&mut s, w[0], w[1], LinkType::FS, 0);
    }
    set(&mut s, &ids);
    let sol = s.compute().expect("the schedule computes");
    assert_eq!(ids.map(|t| dates(&sol, t)), want);
    oracle::assert_matches(&mut s, "late path");
}

#[test]
fn a_finish_limit_two_days_short_makes_the_whole_path_two_days_late() {
    let set = |s: &mut Schedule, t: &[TaskId]| limit(s, t[2], [None, None, Some(late(17, 1, 0).1)]);
    path(
        [8, 6, 6],
        set,
        [late(0, 8, 2), late(8, 6, 2), late(14, 6, 2)],
    );
}

#[test]
fn a_finish_limit_inside_the_path_leaves_its_successor_on_time() {
    let set = |s: &mut Schedule, t: &[TaskId]| limit(s, t[2], [None, None, Some(late(9, 1, 0).1)]);
    let want = [late(0, 5, 2), late(5, 4, 2), late(9, 3, 2), late(12, 2, 0)];
    path([5, 4, 3, 2], set, want);
}

#[test]
fn a_first_task_held_one_day_after_the_start_is_still_two_days_late() {
    let set = |s: &mut Schedule, t: &[TaskId]| {
        s.set_snet(t[0], Some(late(1, 1, 0).0)).unwrap();
        limit(s, t[2], [None, None, Some(late(10, 1, 0).1)]);
    };
    let want = [late(1, 5, 2), late(6, 4, 2), late(10, 3, 2), late(13, 2, 0)];
    path([5, 4, 3, 2], set, want);
}

#[test]
fn a_start_limit_two_days_early_reaches_the_first_task() {
    let set = |s: &mut Schedule, t: &[TaskId]| limit(s, t[1], [Some(late(1, 1, 0).0), None, None]);
    path([3, 2], set, [late(0, 3, 2), late(3, 2, 2)]);
}

/// The pinned task holds its date with float 0; its predecessor cannot finish in time.
#[test]
fn a_finish_pin_two_days_early_makes_the_first_task_two_days_late() {
    let set = |s: &mut Schedule, t: &[TaskId]| pin(s, t[1], Pin::Finish(late(2, 1, 0).1));
    path([3, 2], set, [late(0, 3, 2), late(1, 2, 0)]);
}

/// A late limit or a finish pin at `i64::MIN` (a sentinel, no date) reads as `i64::MIN / 4`: the
/// solve returns, with the dates that limit gives, capped less than a week before it.
#[test]
fn a_late_limit_at_the_lowest_instant_reads_as_a_quarter_of_it() {
    let cases: [fn(&mut Schedule, TaskId, Instant); 3] = [
        |s, t, c| limit(s, t, [Some(c), None, None]),
        |s, t, c| limit(s, t, [None, None, Some(c)]),
        |s, t, c| pin(s, t, Pin::Finish(c)),
    ];
    let floor = i64::MIN / 4;
    for (i, set) in cases.into_iter().enumerate() {
        let solve = |c: Instant| {
            let (mut s, cal) = week();
            let (a, b) = chain(&mut s, cal);
            set(&mut s, b, c);
            let sol = s.compute().expect("a negative float, not an error");
            (dates(&sol, a), dates(&sol, b))
        };
        let (a, b) = solve(i64::MIN);
        assert_eq!((a, b), solve(floor), "case {i}");
        let capped = if i == 0 { b.2 } else { b.3 };
        assert!(capped <= floor && floor - capped < 7 * 1440, "{i}: {b:?}");
        assert!(a.3 <= b.2 && a.4 < 0 && b.4 < 0, "case {i}: {a:?} {b:?}");
    }
}

/// A started task ignores a start limit, a finished task all three (decision 0008); a finish
/// limit still binds the remaining work of a started one.
#[test]
fn progress_overrides_the_limits() {
    let (mut s, cal) = week();
    let (started, finished) = (task(&mut s, cal, D, None), task(&mut s, cal, D, None));
    task(&mut s, cal, 5 * D, None);
    let (d0, d4) = (day(0), day(4));
    s.set_actual_start(started, Some(d0.0)).unwrap();
    limit(&mut s, started, [Some(d0.0), None, None]);
    s.set_actual_start(finished, Some(d0.0)).unwrap();
    s.set_actual_finish(finished, Some(d0.1)).unwrap();
    limit(&mut s, finished, [Some(d4.0), Some(d4.1), Some(d0.0)]);
    let sol = s.compute().unwrap();
    assert_eq!(dates(&sol, started), (d0.0, d0.1, d4.0, d4.1, 4 * D));
    assert_eq!(dates(&sol, finished), (d0.0, d0.1, d0.0, d0.1, 0));
    assert!(!sol.get(finished).is_critical());
    limit(&mut s, started, [None, Some(day(2).1), None]);
    let sol = s.compute().unwrap();
    let d = sol.get(started);
    assert_eq!((d.es, d.ef), (d0.0, day(2).1));
}

/// Set a task's pin.
fn pin(s: &mut Schedule, t: TaskId, pin: Pin) {
    let limit = Limit {
        pin: Some(pin),
        ..Limit::default()
    };
    s.set_limit(t, limit).expect("the task exists");
}

/// `a`, `p` and `b` in a chain of one-day tasks, beside the five-day task.
fn three(s: &mut Schedule, cal: CalendarId) -> [TaskId; 3] {
    let [a, p, b] = [(); 3].map(|()| task(s, cal, D, None));
    task(s, cal, 5 * D, None);
    dep(s, a, p, LinkType::FS, 0);
    dep(s, p, b, LinkType::FS, 0);
    [a, p, b]
}

/// A pin before its predecessor's finish does not move: the pinned task keeps float 0 and the
/// predecessor, which cannot finish in time, gets the negative float.
#[test]
fn pin_before_predecessor_makes_it_negative() {
    let (mut s, cal) = week();
    let [_, p, b] = three(&mut s, cal);
    pin(&mut s, b, Pin::Start(day(1).0));
    let sol = s.compute().unwrap();
    let (d0, d1) = (day(0), day(1));
    assert_eq!(dates(&sol, b), (d1.0, d1.1, d1.0, d1.1, 0));
    assert_eq!(dates(&sol, p), (d1.0, d1.1, d0.0, d0.1, -D));
    assert!(sol.get(p).is_critical() && sol.get(b).is_critical());
}

/// A pin later than the predecessors allow holds too, as a finish pin does, and frees the
/// predecessors' float.
#[test]
fn a_pin_holds_its_date_either_way() {
    let (mut s, cal) = week();
    let (a, b) = chain(&mut s, cal);
    let d3 = day(3);
    for at in [Pin::Start(d3.0), Pin::Finish(d3.1)] {
        pin(&mut s, b, at);
        let sol = s.compute().unwrap();
        assert_eq!(dates(&sol, b), (d3.0, d3.1, d3.0, d3.1, 0), "{at:?}");
        assert_eq!(sol.get(a).float, 2 * D, "{at:?}");
    }
}

/// A started task ignores a start pin, as it ignores start-no-earlier-than (decision 0008).
#[test]
fn a_started_task_ignores_a_start_pin() {
    let (mut s, cal) = week();
    let t = task(&mut s, cal, D, None);
    task(&mut s, cal, 5 * D, None);
    s.set_actual_start(t, Some(day(0).0)).unwrap();
    let before = s.compute().unwrap();
    pin(&mut s, t, Pin::Start(day(2).0));
    assert_eq!(s.compute().unwrap().get(t), before.get(t));
}

/// A pin on a date off working time (midnight, a weekend, after hours) snaps to working time in
/// both passes, so the pinned task keeps float 0: a start takes the next working minute, a
/// finish the end of the last working minute before the date.
#[test]
fn a_pin_off_working_time_keeps_float_zero() {
    let cases = [
        (Pin::Start(at(1, 0, 0)), 1),
        (Pin::Start(at(5, 10, 0)), 7),
        (Pin::Finish(at(2, 0, 0)), 1),
        (Pin::Finish(at(2, 8, 0)), 1),
        (Pin::Finish(at(5, 12, 0)), 4),
    ];
    for (at, n) in cases {
        let (mut s, cal) = week();
        let (_, b) = chain(&mut s, cal);
        pin(&mut s, b, at);
        let sol = s.compute().unwrap();
        let (start, finish) = day(n);
        assert_eq!(dates(&sol, b), (start, finish, start, finish, 0), "{at:?}");
    }
}

/// Mark a task as late as possible, keeping `pin` when given.
fn alap(s: &mut Schedule, t: TaskId, pin: Option<Pin>) {
    let limit = Limit {
        alap: true,
        pin,
        ..Limit::default()
    };
    s.set_limit(t, limit).expect("the task exists");
}

/// `first`, `mid` and `last` in a chain of one-day tasks, with a three-day task also before
/// `last`, beside the five-day task. `mid` has a day of free float (`last` waits for the
/// three-day task) and two of total float.
fn slack(s: &mut Schedule, cal: CalendarId) -> [TaskId; 3] {
    let [first, mid, last] = [(); 3].map(|()| task(s, cal, D, None));
    let long = task(s, cal, 3 * D, None);
    task(s, cal, 5 * D, None);
    dep(s, first, mid, LinkType::FS, 0);
    dep(s, mid, last, LinkType::FS, 0);
    dep(s, long, last, LinkType::FS, 0);
    [first, mid, last]
}

/// An as-late-as-possible task moves by its free float, the room before its successors' early
/// dates, not by its total float: nothing after it moves (as P6 and MS Project place it).
#[test]
fn alap_uses_free_float_not_total() {
    let (mut s, cal) = week();
    let [first, late, after] = slack(&mut s, cal);
    let plain = s.compute().unwrap();
    let (d1, d2, d3) = (day(1), day(2), day(3));
    assert_eq!(dates(&plain, late), (d1.0, d1.1, d3.0, d3.1, 2 * D));
    alap(&mut s, late, None);
    let sol = s.compute().unwrap();
    assert_eq!(dates(&sol, late), (d2.0, d2.1, d3.0, d3.1, D));
    assert_eq!(dates(&sol, after), dates(&plain, after));
    assert_eq!(dates(&sol, first), dates(&plain, first));
    assert_eq!(sol.project_finish, plain.project_finish);
}

/// A chain of as-late-as-possible tasks packs against the successor that does not move, and one
/// with no successor sits at the project finish; a started task and a start-pinned one hold.
#[test]
fn alap_chain_packs_late_and_progress_and_a_pin_hold() {
    let (mut s, cal) = week();
    let [p, a, b] = slack(&mut s, cal);
    let (end, started) = (task(&mut s, cal, D, None), task(&mut s, cal, D, None));
    s.set_actual_start(started, Some(day(0).0)).unwrap();
    for t in [p, a, end, started] {
        alap(&mut s, t, None);
    }
    let sol = s.compute().unwrap();
    assert_eq!(sol.get(p).es, day(1).0);
    assert_eq!(sol.get(a).es, day(2).0);
    assert_eq!(sol.get(end).es, day(4).0);
    assert_eq!(sol.get(b).es, day(3).0);
    assert_eq!(sol.get(started).es, day(0).0);
    alap(&mut s, a, Some(Pin::Start(day(1).0)));
    assert_eq!(s.compute().unwrap().get(a).es, day(1).0);
}

/// A cone, of either kind, on a schedule with an as-late-as-possible task gives what a full
/// compute gives, and lists what moved.
#[test]
fn cones_resolve_a_schedule_with_alap_in_full() {
    let (mut s, cal) = week();
    let [p, a, _] = slack(&mut s, cal);
    alap(&mut s, a, None);
    let mut sol = s.compute().unwrap();
    let was = sol.get(a).es;
    s.set_snet(p, Some(day(2).0)).unwrap();
    let mut moved = Vec::new();
    s.recompute_cone_moved(&mut sol, p, &mut moved).unwrap();
    assert_eq!(sol, s.compute().unwrap());
    assert!(moved.iter().any(|m| m.task == a && m.was_es == was));
    s.set_snet(p, None).unwrap();
    let mut sol = s.compute().unwrap();
    s.set_duration(p, 2 * D).unwrap();
    let how = s
        .recompute_duration_cone_moved(&mut sol, p, &mut moved)
        .unwrap();
    assert_eq!(how, DurationCone::Fallback(FallbackReason::Alap));
    assert_eq!(sol, s.compute().unwrap());
}
