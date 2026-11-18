//! The ranges of decision 0020: a lag, a duration or a date outside its range is refused at each
//! door and nothing changes; the sizes the bounds rest on; and a chain at the bounds computes the
//! dates worked out by hand, in the overflow-checked test profile.

mod common;

use common::{check_invariants, dep, std_cal, task};
use engine::LinkType::FS;
use engine::{
    Calendar, DepId, Dependency, LagCalendar, MAX_DATE, MAX_DURATION, MAX_LAG, MIN_DATE, Pin,
    RangeInput, RangeItem, Schedule, ScheduleError, Task, TaskId, WEEK,
};

/// Two tasks on the standard calendar, linked by dependency 0 when `linked`.
fn base(linked: bool) -> Schedule {
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    let (a, b) = (task(&mut s, cal, 60, None), task(&mut s, cal, 60, None));
    s.set_data_date(Some(480)).expect("in range");
    if linked {
        dep(&mut s, a, b, FS, 0);
    }
    s
}

fn link(lag: i64) -> Dependency {
    let mut d = base(true).dependencies()[0];
    d.lag = lag;
    d
}

type Door<'a> = &'a dyn Fn(&mut Schedule, i64) -> Result<(), ScheduleError>;

/// Each of `bad` is refused with `want(value)` and leaves the tasks, dependencies and data date as
/// they were; each of `good` is accepted.
fn check_door(s0: &Schedule, door: Door, bad: &[i64], want: Want, good: &[i64]) {
    for &v in bad {
        let mut s = s0.clone();
        assert_eq!(door(&mut s, v), Err(want(v)), "value {v}");
        assert_eq!(s.tasks(), s0.tasks(), "tasks after refusing {v}");
        assert_eq!(
            s.dependencies(),
            s0.dependencies(),
            "dependencies after {v}"
        );
        assert_eq!(s.data_date(), s0.data_date(), "data date after {v}");
    }
    for &v in good {
        assert_eq!(door(&mut s0.clone(), v), Ok(()), "value {v}");
    }
}

type Want<'a> = &'a dyn Fn(i64) -> ScheduleError;

#[test]
fn a_duration_outside_its_range_is_refused() {
    let want = |input, id| {
        move |value| match value {
            ..0 => ScheduleError::NegativeDuration(TaskId(id)),
            _ => ScheduleError::OutOfRange {
                input,
                item: RangeItem::Task(TaskId(id)),
                value,
            },
        }
    };
    let bad = [i64::MIN, -1, MAX_DURATION + 1, i64::MAX];
    let s0 = base(false);
    let check = |door: Door, input, id| {
        check_door(&s0, door, &bad, &want(input, id), &[0, MAX_DURATION]);
    };
    let (dur, rem) = (RangeInput::Duration, RangeInput::Remaining);
    let add = |s: &mut Schedule, set: Field, v| {
        let mut t = s.tasks()[0].clone();
        set(&mut t, v);
        s.add_task(t).map(drop)
    };
    check(&|s, v| add(s, |t, v| t.duration = v, v), dur, 2);
    check(&|s, v| s.set_duration(TaskId(0), v), dur, 0);
    check(&|s, v| add(s, |t, v| t.remaining = Some(v), v), rem, 2);
    check(&|s, v| s.set_remaining(TaskId(0), Some(v)), rem, 0);
}

type Field = fn(&mut Task, i64);

/// Each date input is refused at `add_task` and at its setter, and accepted at either bound.
#[test]
fn a_date_outside_its_range_is_refused() {
    use RangeInput::{ActualFinish, ActualStart, DataDate, FinishPin, Fnet, Fnlt};
    use RangeInput::{Resume, Snet, Snlt, StartPin};
    const T: TaskId = TaskId(0);
    let fields: [(RangeInput, Field); 9] = [
        (Snet, |t, v| t.snet = Some(v)),
        (Snlt, |t, v| t.limit.snlt = Some(v)),
        (Fnet, |t, v| t.limit.fnet = Some(v)),
        (Fnlt, |t, v| t.limit.fnlt = Some(v)),
        (Resume, |t, v| t.limit.resume = Some(v)),
        (StartPin, |t, v| t.limit.pin = Some(Pin::Start(v))),
        (FinishPin, |t, v| t.limit.pin = Some(Pin::Finish(v))),
        (ActualStart, |t, v| t.actual_start = Some(v)),
        (ActualFinish, |t, v| t.actual_finish = Some(v)),
    ];
    let bad = [i64::MIN, MIN_DATE - 1, MAX_DATE + 1, i64::MAX];
    let check = |input, item, door: Door| {
        let want = |value| ScheduleError::OutOfRange { input, item, value };
        check_door(&base(false), door, &bad, &want, &[MIN_DATE, MAX_DATE]);
    };
    for (input, set) in fields {
        let with = |s: &Schedule, v| {
            let mut t = s.tasks()[0].clone();
            set(&mut t, v);
            t
        };
        check(input, RangeItem::Task(TaskId(2)), &|s, v| {
            s.add_task(with(s, v)).map(drop)
        });
        check(input, RangeItem::Task(T), &|s, v| {
            let t = with(s, v);
            match input {
                Snet => s.set_snet(T, t.snet),
                ActualStart => s.set_actual_start(T, t.actual_start),
                ActualFinish => s.set_actual_finish(T, t.actual_finish),
                _ => s.set_limit(T, t.limit),
            }
        });
    }
    check(DataDate, RangeItem::Schedule, &|s, v| {
        s.set_data_date(Some(v))
    });
}

#[test]
fn a_lag_outside_its_range_is_refused() {
    let want = |value| ScheduleError::OutOfRange {
        input: RangeInput::Lag,
        item: RangeItem::Dependency(DepId(0)),
        value,
    };
    let bad = [i64::MIN, -MAX_LAG - 1, MAX_LAG + 1, i64::MAX];
    let check =
        |linked, door: Door| check_door(&base(linked), door, &bad, &want, &[-MAX_LAG, 0, MAX_LAG]);
    check(false, &|s, v| s.add_dependency(link(v)).map(drop));
    check(false, &|s, v| s.add_link(link(v)).map(drop));
    check(true, &|s, v| s.set_dependency(DepId(0), FS, v));
    check(true, &|s, v| s.set_dependency_replay(DepId(0), FS, v));
}

/// The sizes the bounds rest on, in `i128` so that no assert can itself overflow.
#[test]
fn the_bounds_leave_room_in_an_instant() {
    let (date, dur, lag): (i128, i128, i128) =
        (MAX_DATE.into(), MAX_DURATION.into(), MAX_LAG.into());
    let chain: i128 = (1 << 32) * (dur + lag);
    assert!(date < 1 << 29 && dur < 1 << 26);
    // The longest chain the engine can hold, in working minutes, under the late floor of 2^61.
    assert!(chain + date < 1 << 59);
    // The same chain in elapsed minutes, on a calendar of `weekly` working minutes a week.
    let elapsed = |weekly: i128| chain * i128::from(WEEK) / weekly + date;
    let (forty_hours, one_minute) = (elapsed(40 * 60), elapsed(1));
    assert!(forty_hours < 1 << 61, "a 40-hour week: {forty_hours}");
    assert!(
        one_minute > i64::MAX.into(),
        "a one-minute week: {one_minute}"
    );
}

/// A calendar, its first start in minutes, and the minutes from a start to the end of a task.
type Shape = (Calendar, i64, i64);

/// Three tasks of `MAX_DURATION` in a chain of finish-to-start links of `lag`, the lag counted in
/// `lag_cal`. Task `i` starts `weeks[i]` weeks after the first start and ends a task length later;
/// every float is 0.
fn run_chain((cal, first, len): Shape, lag_cal: LagCalendar, lag: i64, weeks: [i64; 3]) {
    let mut s = Schedule::new();
    s.set_lag_calendar(lag_cal)
        .expect("the lag calendar is accepted");
    let cal = s.add_calendar(cal);
    let t: Vec<TaskId> = (0..3)
        .map(|_| task(&mut s, cal, MAX_DURATION, None))
        .collect();
    dep(&mut s, t[0], t[1], FS, lag);
    dep(&mut s, t[1], t[2], FS, lag);
    let sol = s.compute().expect("the chain solves");
    check_invariants(&s, &sol);
    let es = weeks.map(|k| first + k * WEEK);
    assert_eq!(
        sol.dates.iter().map(|d| d.es).collect::<Vec<_>>(),
        es,
        "starts"
    );
    assert_eq!(
        sol.dates.iter().map(|d| d.ef).collect::<Vec<_>>(),
        es.map(|e| e + len),
        "ends"
    );
    assert!(sol.dates.iter().all(|d| d.float == 0), "floats");
}

/// The hand arithmetic: `W` is a week of 10,080 minutes and `D` = `L` = 52,704,000 working
/// minutes (100 years of 366 days) is `Q` = 5,228 weeks and 5,760 minutes; on the standard
/// calendar (2,700 working minutes a week) it is `V` = 19,520 weeks. On a one-minute week the
/// working minute `k` is at `k W`, so a task from `k W` ends at `(k + D - 1) W + 1`; on the
/// standard calendar one from Monday 08:00 (minute 480) ends Friday 17:00 (6,780) `V - 1` weeks on.
#[test]
fn a_chain_at_the_bounds_computes_the_dates_worked_out_by_hand() {
    const W: i64 = 10_080;
    const D: i64 = 52_704_000;
    const Q: i64 = 5_228;
    const V: i64 = 19_520;
    assert_eq!((MAX_LAG, D / W, D % W, D / 2700), (D, Q, 5_760, V));
    let w1 = || (one_minute_week(), 0, (D - 1) * W + 1);
    let sd = || (std_cal(&[]), 480, (V - 1) * W + 6_300);
    let (succ, all_day) = (LagCalendar::Successor, LagCalendar::TwentyFourHour);
    // A lag of D working minutes is D weeks: each task starts 2 D weeks after the last. A lag of -D
    // lands on the origin, so every task starts there.
    run_chain(w1(), succ, MAX_LAG, [0, 2 * D, 4 * D]);
    run_chain(w1(), succ, -MAX_LAG, [0; 3]);
    run_chain(sd(), succ, MAX_LAG, [0, 2 * V, 4 * V]);
    run_chain(sd(), succ, -MAX_LAG, [0; 3]);
    // A 24-hour lag is L elapsed minutes: Q weeks and 5,760 minutes, which end inside a week and
    // round up to the next working minute, one week on.
    run_chain(w1(), all_day, MAX_LAG, [0, D + Q, 2 * (D + Q)]);
    run_chain(w1(), all_day, -MAX_LAG, [0, D - 1 - Q, 2 * (D - 1 - Q)]);
}

/// A calendar working the first minute of each Monday.
fn one_minute_week() -> Calendar {
    let mut week: [Vec<(u16, u16)>; 7] = Default::default();
    week[0] = vec![(0, 1)];
    Calendar::new(week, &[]).expect("a valid calendar")
}
