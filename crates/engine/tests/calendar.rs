//! Calendar math at the boundaries. Calendar: Mon-Fri 08:00-17:00; day 0 is
//! the origin Monday, day 2 Wednesday, day 4 Friday, day 5 Saturday, day 7
//! the next Monday. The last tests add exception days: a working Saturday and
//! a 4 h Wednesday, with the dates solved by hand.

mod common;

use common::{at, dep, std_cal, task};
use engine::{Admissible, Calendar, DayPeriod, LinkType, Schedule};

#[test]
fn end_of_day_finishes_at_the_period_end() {
    let c = std_cal(&[]);
    assert_eq!(c.add(at(0, 16, 0), 60), at(0, 17, 0), "not 08:00 Tuesday");
    assert_eq!(c.add(at(0, 16, 0), 61), at(1, 8, 1));
    assert_eq!(c.add(at(0, 8, 0), 540), at(0, 17, 0));
}

#[test]
fn weekend_is_skipped_and_starts_snap_forward() {
    let c = std_cal(&[]);
    assert_eq!(
        c.add(at(4, 16, 0), 60),
        at(4, 17, 0),
        "done 17:00 Friday stays Friday"
    );
    assert_eq!(c.add(at(4, 16, 0), 120), at(7, 9, 0));
    assert_eq!(c.next_working(at(5, 12, 0)), at(7, 8, 0));
    assert_eq!(c.next_working(at(4, 17, 0)), at(7, 8, 0));
    assert_eq!(c.next_working(at(4, 16, 59)), at(4, 16, 59));
    // from inside a weekend, forward counting starts Monday 08:00
    assert_eq!(c.add(at(6, 3, 0), 30), at(7, 8, 30));
}

#[test]
fn holiday_is_skipped() {
    let c = std_cal(&[7]); // the second Monday
    assert!(!c.is_working(at(7, 10, 0)));
    assert_eq!(c.add(at(4, 16, 0), 120), at(8, 9, 0));
    assert_eq!(c.next_working(at(5, 0, 0)), at(8, 8, 0));
    assert_eq!(c.add(at(8, 9, 0), -120), at(4, 16, 0));
    assert_eq!(c.working_between(at(4, 8, 0), at(8, 17, 0)), 2 * 540);
}

#[test]
fn negative_lag_across_a_weekend() {
    let c = std_cal(&[]);
    assert_eq!(c.add(at(7, 9, 0), -120), at(4, 16, 0));
    assert_eq!(c.add(at(7, 8, 0), -60), at(4, 16, 0));
    // from non-working Saturday noon, back an hour is Friday 16:00
    assert_eq!(c.add(at(5, 12, 0), -60), at(4, 16, 0));
    // the largest instant with the minutes in between: Monday 08:00, not Friday 17:00
    assert_eq!(c.add(at(7, 9, 0), -60), at(7, 8, 0));
}

#[test]
fn clamp_at_origin() {
    let c = std_cal(&[]);
    assert_eq!(c.add(at(0, 9, 0), -60), at(0, 8, 0));
    // `add` runs on before the origin, in the weekly pattern: one minute more is Friday 16:59
    assert_eq!(c.add(at(0, 9, 0), -61), at(-3, 16, 59));
    let far = c.add(at(2, 9, 0), -100_000);
    assert_eq!(c.working_between(far, at(2, 9, 0)), 100_000);
    // a negative lag that would reach before the origin schedules at the origin
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, cal, 60, None);
    let b = task(&mut s, cal, 60, None);
    dep(&mut s, a, b, LinkType::FS, -600);
    let sol = s.compute().unwrap();
    assert_eq!(
        sol.get(b).es,
        at(0, 8, 0),
        "clamped to origin, then snapped to the first working minute"
    );
    common::check_invariants(&s, &sol);
}

#[test]
fn zero_lag_is_identity_even_in_non_working_time() {
    let c = std_cal(&[]);
    for t in [0, at(5, 12, 0), at(4, 17, 0), at(3, 9, 30)] {
        assert_eq!(c.add(t, 0), t);
    }
}

#[test]
fn prev_working_and_period_ends() {
    let c = std_cal(&[]);
    assert_eq!(c.prev_working(at(5, 12, 0)), Some(at(4, 16, 59)));
    assert_eq!(c.prev_period_end(at(5, 12, 0)), Some(at(4, 17, 0)));
    assert_eq!(c.prev_working(at(0, 7, 0)), None);
    assert_eq!(c.prev_period_end(at(0, 8, 0)), None);
    assert!(c.is_period_end(at(4, 17, 0)));
    assert!(!c.is_period_end(at(4, 8, 0)));
}

#[test]
fn next_and_prev_working_before_the_origin() {
    let c = std_cal(&[]);
    // Origin Monday 00:00 is not working; the first working minute is 08:00.
    let first = at(0, 8, 0);
    // before the origin: nothing precedes, so next is the first working instant
    for t in [i64::MIN, -100_000, -1441, -1] {
        assert_eq!(c.next_working(t), first, "next_working({t})");
        assert_eq!(c.prev_working(t), None, "prev_working({t})");
    }
    // at the origin
    assert_eq!(c.next_working(0), first);
    assert_eq!(c.prev_working(0), None);
    // one after the origin, inside the same non-working stretch
    assert_eq!(c.next_working(1), first);
    assert_eq!(c.prev_working(1), None);
    // a calendar that is working at the origin: next is the origin itself
    let mut week = c.week().clone();
    week[0] = vec![(0, 60)];
    let night = engine::Calendar::new(week, &[]).unwrap();
    assert_eq!(night.next_working(-1), 0);
    assert_eq!(night.next_working(0), 0);
    assert_eq!(night.prev_working(-1), None);
    assert_eq!(night.prev_working(0), Some(0));
}

#[test]
fn prev_period_end_before_the_origin() {
    let c = std_cal(&[]);
    for t in [i64::MIN, -100_000, -1441, -1] {
        assert_eq!(c.prev_period_end(t), None, "prev_period_end({t})");
    }
    assert_eq!(c.prev_period_end(0), None);
    assert_eq!(c.prev_period_end(1), None);

    let mut week = c.week().clone();
    week[0] = vec![(0, 60)];
    let night = engine::Calendar::new(week, &[]).unwrap();
    assert_eq!(night.prev_period_end(-1), None);
    assert_eq!(night.prev_period_end(0), None);
    assert_eq!(night.prev_period_end(1), Some(1));
    assert_eq!(night.prev_period_end(60), Some(60));

    // An admissible start always precedes: before the origin the weekly pattern runs on.
    let milestone = Admissible::new(&c, 0);
    for t in [-1, 0] {
        assert_eq!(milestone.prev(t), at(-3, 17, 0), "milestone.prev({t})");
    }
    // An instant before `i64::MIN / 2` reads as it: a working minute (a Wednesday, 14:56) here.
    assert_eq!(milestone.prev(i64::MIN), i64::MIN / 2);
    let night_milestone = Admissible::new(&night, 0);
    assert_eq!(night_milestone.prev(-1), at(-3, 17, 0));
    assert_eq!(night_milestone.prev(0), 0);
}

#[test]
fn milestones_may_sit_on_a_period_end() {
    let c = std_cal(&[]);
    let milestone = Admissible::new(&c, 0);
    let timed = Admissible::new(&c, 60);
    assert_eq!(milestone.next(at(4, 17, 0)), at(4, 17, 0));
    assert_eq!(timed.next(at(4, 17, 0)), at(7, 8, 0));
    assert_eq!(milestone.fin(at(4, 17, 0)), at(4, 17, 0));
    // in a schedule: a task done 17:00 Friday, then a milestone and a task
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, cal, 540, Some(at(4, 0, 0)));
    let ms = task(&mut s, cal, 0, None);
    let b = task(&mut s, cal, 60, None);
    dep(&mut s, a, ms, LinkType::FS, 0);
    dep(&mut s, a, b, LinkType::FS, 0);
    let sol = s.compute().unwrap();
    assert_eq!(sol.get(a).ef, at(4, 17, 0));
    assert_eq!(sol.get(ms).es, at(4, 17, 0));
    assert_eq!(sol.get(b).es, at(7, 8, 0));
    common::check_invariants(&s, &sol);
}

#[test]
fn long_spans_agree_with_counting() {
    let c = std_cal(&[3, 10, 11, 30, 31, 32, 200]);
    let start = at(1, 10, 15);
    for w in [1, 539, 540, 5_000, 50_000, 123_456] {
        let u = c.add(start, w);
        assert_eq!(c.working_between(start, u), w, "w = {w}");
        assert!(
            c.is_working(u - 1),
            "w = {w}: lands just after a working minute"
        );
        let back = c.add(u, -w);
        assert_eq!(c.working_between(back, u), w, "w = -{w}");
        assert!(c.is_working(back));
    }
}

/// Mon-Fri 08:00-17:00 with exception days `(day, periods)`.
fn std_with(exceptions: &[(i64, Vec<DayPeriod>)]) -> Calendar {
    Calendar::with_exceptions(std_cal(&[]).week().clone(), exceptions)
        .expect("the exceptions form a valid calendar")
}

#[test]
fn a_working_saturday_carries_the_work() {
    let c = std_with(&[(5, vec![(480, 720)])]);
    assert!(c.is_working(at(5, 8, 0)));
    assert!(c.is_working(at(5, 11, 59)));
    assert!(!c.is_working(at(5, 7, 59)));
    assert!(!c.is_working(at(5, 12, 0)));
    assert!(!c.is_working(at(6, 10, 0)), "Sunday stays off");
    assert!(!c.is_working(at(12, 10, 0)), "only that one Saturday");
    // 4 h on Friday afternoon, 4 h on Saturday morning: Saturday 12:00, where
    // the plain week gives Monday 12:00.
    assert_eq!(c.add(at(4, 13, 0), 480), at(5, 12, 0));
    assert_eq!(std_cal(&[]).add(at(4, 13, 0), 480), at(7, 12, 0));
    // A task that fills Friday still ends 17:00 Friday, not 08:00 Saturday.
    assert_eq!(c.add(at(4, 8, 0), 540), at(4, 17, 0));
    assert_eq!(c.next_working(at(4, 17, 0)), at(5, 8, 0));
    assert_eq!(c.add(at(5, 12, 0), 1), at(7, 8, 1));
    // Backward from Monday 08:30: 30 minutes of Monday, 30 of Saturday.
    assert_eq!(c.add(at(7, 8, 30), -60), at(5, 11, 30));
    assert_eq!(c.prev_period_end(at(6, 12, 0)), Some(at(5, 12, 0)));
    assert_eq!(
        c.working_between(at(4, 8, 0), at(7, 17, 0)),
        540 + 240 + 540
    );

    let mut s = Schedule::new();
    let cal = s.add_calendar(c);
    let a = task(&mut s, cal, 480, Some(at(4, 13, 0)));
    let b = task(&mut s, cal, 60, None);
    dep(&mut s, a, b, LinkType::FS, 0);
    let sol = s.compute().unwrap();
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(4, 13, 0), at(5, 12, 0)));
    assert_eq!((sol.get(b).es, sol.get(b).ef), (at(7, 8, 0), at(7, 9, 0)));
    common::check_invariants(&s, &sol);
    common::oracle::assert_matches(&mut s, "working Saturday");
}

#[test]
fn a_four_hour_day_pushes_an_eight_hour_task_into_the_next_day() {
    let c = std_with(&[(2, vec![(480, 720)])]);
    assert_eq!(c.add(at(2, 8, 0), 480), at(3, 12, 0));
    assert_eq!(std_cal(&[]).add(at(2, 8, 0), 480), at(2, 16, 0));
    assert!(c.is_working(at(2, 11, 59)));
    assert!(!c.is_working(at(2, 12, 0)));
    assert_eq!(c.next_working(at(2, 12, 0)), at(3, 8, 0));
    assert_eq!(c.add(at(3, 9, 0), -120), at(2, 11, 0));
    assert_eq!(
        c.working_between(at(1, 8, 0), at(3, 17, 0)),
        540 + 240 + 540
    );

    let mut s = Schedule::new();
    let cal = s.add_calendar(c);
    let a = task(&mut s, cal, 480, Some(at(2, 8, 0)));
    let sol = s.compute().unwrap();
    assert_eq!((sol.get(a).es, sol.get(a).ef), (at(2, 8, 0), at(3, 12, 0)));
    common::check_invariants(&s, &sol);
    common::oracle::assert_matches(&mut s, "4 h day");
}

#[test]
fn a_milestone_may_sit_on_a_shortened_day_period_end() {
    let c = std_with(&[(2, vec![(480, 720)])]);
    assert!(c.is_period_end(at(2, 12, 0)));
    assert!(
        !c.is_period_end(at(2, 17, 0)),
        "the pattern's end is gone that day"
    );
    assert_eq!(c.prev_period_end(at(2, 16, 0)), Some(at(2, 12, 0)));
    let milestone = Admissible::new(&c, 0);
    let timed = Admissible::new(&c, 60);
    assert_eq!(milestone.next(at(2, 12, 0)), at(2, 12, 0));
    assert_eq!(timed.next(at(2, 12, 0)), at(3, 8, 0));

    let mut s = Schedule::new();
    let cal = s.add_calendar(c);
    let a = task(&mut s, cal, 240, Some(at(2, 8, 0)));
    let ms = task(&mut s, cal, 0, None);
    let b = task(&mut s, cal, 60, None);
    dep(&mut s, a, ms, LinkType::FS, 0);
    dep(&mut s, a, b, LinkType::FS, 0);
    let sol = s.compute().unwrap();
    assert_eq!(sol.get(a).ef, at(2, 12, 0));
    assert_eq!(sol.get(ms).es, at(2, 12, 0));
    assert_eq!(sol.get(b).es, at(3, 8, 0));
    common::check_invariants(&s, &sol);
    common::oracle::assert_matches(&mut s, "milestone on a 4 h day");
}
