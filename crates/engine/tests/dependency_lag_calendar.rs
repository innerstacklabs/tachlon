//! The per-dependency lag calendar (gs#692, `Schedule::set_dependency_lag_calendar`): one
//! dependency counts its lag in a calendar of its own, the others follow the schedule's choice.
//! The schedule-wide choice is covered in `lag_calendar.rs`; the fuzz in `common/fuzz.rs` draws
//! own calendars too, so the oracle, the cone and the duration cone tests compare them.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

mod common;

use common::{at, check_invariants, mornings_cal, six_cal, std_cal, task};
use engine::LinkType::FS;
use engine::{
    CalendarId, DepId, Dependency, LagCalendar, Schedule, ScheduleError, Solution, TaskId,
};

/// P1 and P2 (Mon-Fri, five days, each ends Fri 17:00) -> Q (Mon-Sat, one hour) by FS with a
/// 16 h lag, as in `lag_calendar.rs`. Returns the schedule, P1, P2, Q and the two links.
fn two_links() -> (Schedule, TaskId, TaskId, TaskId, DepId, DepId) {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    s.add_calendar(mornings_cal());
    let p1 = task(&mut s, std, 5 * 540, None);
    let p2 = task(&mut s, std, 5 * 540, None);
    let q = task(&mut s, six, 60, None);
    let mut link = |pred| {
        s.add_dependency(Dependency {
            pred,
            succ: q,
            kind: FS,
            lag: 960,
        })
        .expect("the link is valid")
    };
    let (a, b) = (link(p1), link(p2));
    (s, p1, p2, q, a, b)
}

/// Where Q starts when its one link counts the 16 h lag in `choice` (the table in
/// `lag_calendar.rs`).
fn start_under(choice: LagCalendar) -> i64 {
    match choice {
        LagCalendar::Successor => at(7, 15, 0),
        LagCalendar::Predecessor => at(8, 15, 0),
        LagCalendar::TwentyFourHour => at(5, 9, 0),
        LagCalendar::Fixed(_) => at(8, 12, 0),
    }
}

const CHOICES: [LagCalendar; 4] = [
    LagCalendar::Successor,
    LagCalendar::Predecessor,
    LagCalendar::TwentyFourHour,
    LagCalendar::Fixed(CalendarId(2)),
];

fn solve(s: &mut Schedule) -> Solution {
    let sol = s.compute().expect("the schedule computes");
    check_invariants(s, &sol);
    sol
}

#[test]
fn a_dependency_follows_the_schedule_until_it_is_given_its_own() {
    let (mut s, _, _, _, a, _) = two_links();
    assert_eq!(s.dependency_lag_calendar(a), None);
    s.set_dependency_lag_calendar(a, Some(LagCalendar::Predecessor))
        .unwrap();
    assert_eq!(s.dependency_lag_calendar(a), Some(LagCalendar::Predecessor));
    s.set_dependency_lag_calendar(a, None).unwrap();
    assert_eq!(s.dependency_lag_calendar(a), None);
    // A slot past the end reads as "follows the schedule", not as a panic.
    assert_eq!(s.dependency_lag_calendar(DepId(99)), None);
}

#[test]
fn an_own_calendar_counts_the_lag_as_the_schedule_wide_choice_would() {
    for choice in CHOICES {
        let (mut s, _, p2, q, a, b) = two_links();
        s.set_dependency_lag_calendar(a, Some(choice)).unwrap();
        s.set_dependency_lag_calendar(b, Some(choice)).unwrap();
        let sol = solve(&mut s);
        assert_eq!(sol.get(q).es, start_under(choice), "{choice:?}");
        assert!(s.driving_predecessors(&sol, q).contains(&p2), "{choice:?}");
    }
}

#[test]
fn each_dependency_counts_in_its_own_calendar() {
    let (mut s, p1, p2, q, a, b) = two_links();
    // The schedule's choice (Successor) puts Q on Monday 15:00. Link a alone asks for the
    // predecessor's calendar, which is later; link b follows the schedule.
    s.set_dependency_lag_calendar(a, Some(LagCalendar::Predecessor))
        .unwrap();
    let sol = solve(&mut s);
    assert_eq!(sol.get(q).es, at(8, 15, 0));
    assert_eq!(s.driving_predecessors(&sol, q), vec![p1]);
    // Link a back on the schedule's choice: both links give Monday 15:00 and both drive.
    s.set_dependency_lag_calendar(a, None).unwrap();
    let sol = solve(&mut s);
    assert_eq!(sol.get(q).es, at(7, 15, 0));
    assert_eq!(s.driving_predecessors(&sol, q), vec![p1, p2]);
    // Link b alone asks for it: link a is not touched.
    s.set_dependency_lag_calendar(b, Some(LagCalendar::Predecessor))
        .unwrap();
    let sol = solve(&mut s);
    assert_eq!(sol.get(q).es, at(8, 15, 0));
    assert_eq!(s.driving_predecessors(&sol, q), vec![p2]);
}

#[test]
fn an_own_calendar_wins_over_a_later_schedule_wide_choice() {
    let (mut s, _, _, q, a, b) = two_links();
    s.set_dependency_lag_calendar(a, Some(LagCalendar::TwentyFourHour))
        .unwrap();
    s.set_dependency_lag_calendar(b, Some(LagCalendar::TwentyFourHour))
        .unwrap();
    for choice in CHOICES {
        s.set_lag_calendar(choice).unwrap();
        assert_eq!(solve(&mut s).get(q).es, at(5, 9, 0), "{choice:?}");
    }
}

#[test]
fn the_slack_follows_the_own_calendar() {
    // Link b counts in the predecessor's calendar and sets Q's start (Tuesday 15:00). Link a
    // counts 16 elapsed hours, so P1 can finish a working day late and still meet Q. The float
    // reaches P1 only through the backward pass reading link a's own calendar.
    let (mut s, p1, p2, _, a, b) = two_links();
    s.set_dependency_lag_calendar(a, Some(LagCalendar::TwentyFourHour))
        .unwrap();
    s.set_dependency_lag_calendar(b, Some(LagCalendar::Predecessor))
        .unwrap();
    let sol = solve(&mut s);
    assert!(sol.get(p1).float > 0, "{:?}", sol.get(p1));
    assert_eq!(sol.get(p2).float, 0);
}

#[test]
fn a_refused_setting_changes_nothing() {
    let (mut s, _, _, q, a, _) = two_links();
    s.set_dependency_lag_calendar(a, Some(LagCalendar::Predecessor))
        .unwrap();
    for bad in [CalendarId(3), CalendarId(u32::MAX)] {
        assert_eq!(
            s.set_dependency_lag_calendar(a, Some(LagCalendar::Fixed(bad))),
            Err(ScheduleError::UnknownCalendar(bad))
        );
        assert_eq!(s.dependency_lag_calendar(a), Some(LagCalendar::Predecessor));
    }
    assert_eq!(
        s.set_dependency_lag_calendar(DepId(2), Some(LagCalendar::Successor)),
        Err(ScheduleError::UnknownDependency(DepId(2)))
    );
    assert_eq!(
        s.set_dependency_lag_calendar(DepId(2), None),
        Err(ScheduleError::UnknownDependency(DepId(2)))
    );
    assert_eq!(solve(&mut s).get(q).es, at(8, 15, 0));
}

#[test]
fn setting_one_keeps_the_cached_order_and_a_clone_keeps_the_setting() {
    let (mut s, _, _, q, a, _) = two_links();
    s.topological_order().unwrap();
    s.set_dependency_lag_calendar(a, Some(LagCalendar::Predecessor))
        .unwrap();
    assert!(s.has_cached_order());
    let mut copy = s.clone();
    assert_eq!(
        copy.dependency_lag_calendar(a),
        Some(LagCalendar::Predecessor)
    );
    assert_eq!(copy.compute().unwrap().get(q).es, at(8, 15, 0));
}

#[test]
fn a_removed_link_keeps_its_own_calendar_for_the_undo() {
    let (mut s, _, _, q, a, b) = two_links();
    s.set_dependency_lag_calendar(a, Some(LagCalendar::Predecessor))
        .unwrap();
    s.remove_dependency(a).unwrap();
    s.remove_dependency(b).unwrap();
    s.restore_dependency(b).unwrap();
    assert_eq!(s.compute().unwrap().get(q).es, at(7, 15, 0));
    s.restore_dependency(a).unwrap();
    assert_eq!(s.dependency_lag_calendar(a), Some(LagCalendar::Predecessor));
    assert_eq!(solve(&mut s).get(q).es, at(8, 15, 0));
}

/// Without this the fuzz could draw own calendars that never change a date and the oracle
/// comparison would pass vacuously.
#[test]
fn the_fuzz_draws_own_calendars_that_change_dates() {
    let (mut with, mut changes) = (0, 0);
    for seed in 0..400 {
        let mut s = common::fuzz::schedule(seed);
        let mut plain = s.clone();
        let mut any = false;
        for i in 0..s.dependencies().len() as u32 {
            any |= s.dependency_lag_calendar(DepId(i)).is_some();
            plain.set_dependency_lag_calendar(DepId(i), None).unwrap();
        }
        with += any as i32;
        if let (Ok(own), Ok(base)) = (s.compute(), plain.compute()) {
            changes += (own != base) as i32;
        }
    }
    assert!(with > 200, "schedules with an own calendar: {with}");
    assert!(changes > 20, "own calendars that change a date: {changes}");
}
