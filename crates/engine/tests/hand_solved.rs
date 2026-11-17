//! Hand-solved schedules. Every expected date below was worked out by hand
//! and is written into the test; the brute-force oracle then re-checks each
//! schedule as a second opinion.

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

use common::{
    assert_critical_chain, assert_dates, at, check_invariants, cont_cal, dep, mornings_cal, oracle,
    six_cal, std_cal, task, want,
};
use engine::LinkType::{FF, FS, SF, SS};
use engine::{LagCalendar, Schedule, TaskId};

const H: i64 = 60;

fn finish(s: &mut Schedule) -> engine::Solution {
    let sol = s.compute().expect("computes");
    check_invariants(s, &sol);
    oracle::assert_matches(s, "hand-solved");
    sol
}

/// Textbook 1 (24/7 calendar, hours):
/// A(3) -> B(4), A -> C(2), B -> D(5), C -> D, D -> E(1). Critical A-B-D-E.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn textbook_one() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let a = task(&mut s, c, 3 * H, None);
    let b = task(&mut s, c, 4 * H, None);
    let cc = task(&mut s, c, 2 * H, None);
    let d = task(&mut s, c, 5 * H, None);
    let e = task(&mut s, c, H, None);
    for (p, q) in [(a, b), (a, cc), (b, d), (cc, d), (d, e)] {
        dep(&mut s, p, q, FS, 0);
    }
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, 13 * H);
    assert_dates(&sol, a, want(0, 3 * H, 0, 3 * H, 0));
    assert_dates(&sol, b, want(3 * H, 7 * H, 3 * H, 7 * H, 0));
    assert_dates(&sol, cc, want(3 * H, 5 * H, 5 * H, 7 * H, 2 * H));
    assert_dates(&sol, d, want(7 * H, 12 * H, 7 * H, 12 * H, 0));
    assert_dates(&sol, e, want(12 * H, 13 * H, 12 * H, 13 * H, 0));
    assert_critical_chain(&s, &sol, &[a, b, d, e]);
}

/// Textbook 2: two start tasks.
/// A(4) -> C(3) -> F(2); B(6) -> D(5) -> F; B -> G(4). Critical B-D-F.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn textbook_two() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let a = task(&mut s, c, 4 * H, None);
    let b = task(&mut s, c, 6 * H, None);
    let cc = task(&mut s, c, 3 * H, None);
    let d = task(&mut s, c, 5 * H, None);
    let f = task(&mut s, c, 2 * H, None);
    let g = task(&mut s, c, 4 * H, None);
    for (p, q) in [(a, cc), (b, d), (cc, f), (d, f), (b, g)] {
        dep(&mut s, p, q, FS, 0);
    }
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, 13 * H);
    assert_dates(&sol, a, want(0, 4 * H, 4 * H, 8 * H, 4 * H));
    assert_dates(&sol, b, want(0, 6 * H, 0, 6 * H, 0));
    assert_dates(&sol, cc, want(4 * H, 7 * H, 8 * H, 11 * H, 4 * H));
    assert_dates(&sol, d, want(6 * H, 11 * H, 6 * H, 11 * H, 0));
    assert_dates(&sol, f, want(11 * H, 13 * H, 11 * H, 13 * H, 0));
    assert_dates(&sol, g, want(6 * H, 10 * H, 9 * H, 13 * H, 3 * H));
    assert_critical_chain(&s, &sol, &[b, d, f]);
}

/// Textbook 3: start and end milestones.
/// S(0) -> X(5) -> Z(2) -> End(0); S -> Y(3) -> Z; Y -> W(6) -> End.
/// Critical S-Y-W-End.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn textbook_three_with_milestones() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let st = task(&mut s, c, 0, None);
    let x = task(&mut s, c, 5 * H, None);
    let y = task(&mut s, c, 3 * H, None);
    let z = task(&mut s, c, 2 * H, None);
    let w = task(&mut s, c, 6 * H, None);
    let end = task(&mut s, c, 0, None);
    for (p, q) in [(st, x), (st, y), (x, z), (y, z), (y, w), (z, end), (w, end)] {
        dep(&mut s, p, q, FS, 0);
    }
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, 9 * H);
    assert_dates(&sol, st, want(0, 0, 0, 0, 0));
    assert_dates(&sol, x, want(0, 5 * H, 2 * H, 7 * H, 2 * H));
    assert_dates(&sol, y, want(0, 3 * H, 0, 3 * H, 0));
    assert_dates(&sol, z, want(5 * H, 7 * H, 7 * H, 9 * H, 2 * H));
    assert_dates(&sol, w, want(3 * H, 9 * H, 3 * H, 9 * H, 0));
    assert_dates(&sol, end, want(9 * H, 9 * H, 9 * H, 9 * H, 0));
    assert_critical_chain(&s, &sol, &[st, y, w, end]);
}

/// Mixed calendars, SS with positive lag and FS with negative lag.
/// std = Mon-Fri 08:00-17:00, six = Mon-Sat 08:00-17:00.
///   A std 600 min: Mon 08:00 -> Tue 09:00 (540 Mon + 60 Tue).
///   B six 300, SS from A lag +120 (six): es = Mon 10:00, ef Mon 15:00.
///   C std 240, FS from B lag -120 (std): es = Mon 13:00, ef Mon 17:00.
/// PF = Tue 09:00.
///   C: fin <= Tue 09:00 -> ls Mon 14:00, float 60.
///   B: add(std, fin, -120) <= Mon 14:00 -> fin <= Mon 16:00 -> ls Mon 11:00, float 60.
///   A: add(six, x, 120) <= Mon 11:00 and fin <= PF -> ls Mon 08:00, float 0.
#[test]
fn mixed_ss_positive_fs_negative() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let a = task(&mut s, std, 600, None);
    let b = task(&mut s, six, 300, None);
    let c = task(&mut s, std, 240, None);
    dep(&mut s, a, b, SS, 120);
    dep(&mut s, b, c, FS, -120);
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, at(1, 9, 0));
    assert_dates(
        &sol,
        a,
        want(at(0, 8, 0), at(1, 9, 0), at(0, 8, 0), at(1, 9, 0), 0),
    );
    assert_dates(
        &sol,
        b,
        want(at(0, 10, 0), at(0, 15, 0), at(0, 11, 0), at(0, 16, 0), 60),
    );
    assert_dates(
        &sol,
        c,
        want(at(0, 13, 0), at(0, 17, 0), at(0, 14, 0), at(1, 9, 0), 60),
    );
}

/// FF with negative lag, SF with positive lag landing on a weekend, SNET
/// driving (A) and not driving (B).
///   A std 360, SNET Fri 13:00 (driving): Fri 13:00 -> Mon 10:00.
///   B six 180, FF from A lag -60 (six), SNET Wed 08:00 (not driving):
///     fin >= add(six, Mon 10:00, -60) = Mon 09:00 -> es Sat 15:00, ef Mon 09:00.
///   C std 120, SF from B lag +30 (std): fin >= add(std, Sat 15:00, 30) = Mon 08:30
///     -> es Fri 15:30 (90 Fri + 30 Mon), ef Mon 08:30.
/// PF = Mon 10:00.
///   C: fin <= Mon 10:00 -> ls Mon 08:00, lf Mon 10:00, float 90.
///   B: add(std, x, 30) <= Mon 10:00 and fin <= Mon 10:00 -> ls Sat 16:00, float 60.
///   A: add(six, fin, -60) <= Mon 10:00 and fin <= PF -> ls Fri 13:00, float 0.
#[test]
fn mixed_ff_negative_sf_positive_snet() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let a = task(&mut s, std, 360, Some(at(4, 13, 0)));
    let b = task(&mut s, six, 180, Some(at(2, 8, 0)));
    let c = task(&mut s, std, 120, None);
    dep(&mut s, a, b, FF, -60);
    dep(&mut s, b, c, SF, 30);
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, at(7, 10, 0));
    assert_dates(
        &sol,
        a,
        want(at(4, 13, 0), at(7, 10, 0), at(4, 13, 0), at(7, 10, 0), 0),
    );
    assert_dates(
        &sol,
        b,
        want(at(5, 15, 0), at(7, 9, 0), at(5, 16, 0), at(7, 10, 0), 60),
    );
    assert_dates(
        &sol,
        c,
        want(at(4, 15, 30), at(7, 8, 30), at(7, 8, 0), at(7, 10, 0), 90),
    );
}

/// FS with positive lag across a weekend and a holiday, a milestone, and SS
/// with a negative lag into a 24/7 calendar.
///   A std 480, SNET Fri 08:00: Fri 08:00 -> Fri 16:00.
///   B milestone on std with holiday Mon (day 7), FS from A lag +120:
///     add(std+hol, Fri 16:00, 120) = 60 Fri + 60 Tue = Tue 09:00.
///   C 24/7 60, SS from B lag -1440: es Mon 09:00, ef Mon 10:00.
/// PF = Tue 09:00.
///   C: ls Tue 08:00, float 1380.  B: ls Tue 09:00, float 0.  A: ls Fri 08:00, float 0.
#[test]
fn mixed_fs_lag_over_weekend_and_holiday_milestone() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let hol = s.add_calendar(std_cal(&[7]));
    let cont = s.add_calendar(cont_cal());
    let a = task(&mut s, std, 480, Some(at(4, 8, 0)));
    let b = task(&mut s, hol, 0, None);
    let c = task(&mut s, cont, 60, None);
    dep(&mut s, a, b, FS, 120);
    dep(&mut s, b, c, SS, -1440);
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, at(8, 9, 0));
    assert_dates(
        &sol,
        a,
        want(at(4, 8, 0), at(4, 16, 0), at(4, 8, 0), at(4, 16, 0), 0),
    );
    assert_dates(
        &sol,
        b,
        want(at(8, 9, 0), at(8, 9, 0), at(8, 9, 0), at(8, 9, 0), 0),
    );
    assert_dates(
        &sol,
        c,
        want(at(7, 9, 0), at(7, 10, 0), at(8, 8, 0), at(8, 9, 0), 1380),
    );
    assert_critical_chain(&s, &sol, &[a, b]);
}

/// Textbook 1: A finishes at hour 3; B and C finish at hours 7 and 5,
/// D starts at hour 7, and E starts when D finishes at hour 12.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn textbook_one_driving_dependencies() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let a = task(&mut s, cal, 3 * H, None);
    let b = task(&mut s, cal, 4 * H, None);
    let c = task(&mut s, cal, 2 * H, None);
    let d = task(&mut s, cal, 5 * H, None);
    let e = task(&mut s, cal, H, None);
    for (pred, succ) in [(a, b), (a, c), (b, d), (c, d), (d, e)] {
        dep(&mut s, pred, succ, FS, 0);
    }
    let sol = finish(&mut s);

    assert_eq!(s.driving_predecessors(&sol, d), vec![b]);
    assert_eq!(s.driving_predecessors(&sol, e), vec![d]);
    assert_eq!(s.driving_predecessors(&sol, b), vec![a]);
    assert_eq!(s.driving_predecessors(&sol, c), vec![a]);
    assert_eq!(s.driving_predecessors(&sol, a), vec![]);
    assert_eq!(s.driven_successors(&sol, a), vec![b, c]);
    assert_eq!(s.driven_successors(&sol, b), vec![d]);
    assert_eq!(s.driven_successors(&sol, c), vec![]);
    assert_eq!(s.driven_successors(&sol, e), vec![]);
}

/// Textbook 1 with C lasting four hours: B and C both finish at hour 7,
/// so both independently set D's hour-7 early start.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn textbook_one_two_equal_drivers() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let a = task(&mut s, cal, 3 * H, None);
    let b = task(&mut s, cal, 4 * H, None);
    let c = task(&mut s, cal, 4 * H, None);
    let d = task(&mut s, cal, 5 * H, None);
    let e = task(&mut s, cal, H, None);
    for (pred, succ) in [(a, b), (a, c), (b, d), (c, d), (d, e)] {
        dep(&mut s, pred, succ, FS, 0);
    }
    let sol = finish(&mut s);

    assert_eq!(s.driving_predecessors(&sol, d), vec![b, c]);
    assert_eq!(s.driven_successors(&sol, b), vec![d]);
    assert_eq!(s.driven_successors(&sol, c), vec![d]);
}

/// std = Mon-Fri 08:00-17:00; cont = 24/7.
///   X std 540, SNET Fri 08:00: es Fri 08:00, ef Fri 17:00.
///   Y cont 600, SNET Sat 10:00: es Sat 10:00, ef Sat 20:00.
///   W cont 60: es Sat 20:00, ef Sat 21:00. PF = Sat 21:00.
/// Late dates: W ls Sat 20:00; Y ls Sat 10:00; the latest std start for X
/// finishing by Sat 10:00 is Fri 08:00 (finishing Fri 17:00). All float is 0.
#[test]
fn longest_path_differs_from_zero_float_with_snet() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let cont = s.add_calendar(cont_cal());
    let x = task(&mut s, std, 540, Some(at(4, 8, 0)));
    let y = task(&mut s, cont, 600, Some(at(5, 10, 0)));
    let w = task(&mut s, cont, 60, None);
    dep(&mut s, x, y, FS, 0);
    dep(&mut s, y, w, FS, 0);
    let sol = finish(&mut s);

    assert_eq!(sol.project_finish, at(5, 21, 0));
    assert_dates(
        &sol,
        x,
        want(at(4, 8, 0), at(4, 17, 0), at(4, 8, 0), at(4, 17, 0), 0),
    );
    assert_dates(
        &sol,
        y,
        want(at(5, 10, 0), at(5, 20, 0), at(5, 10, 0), at(5, 20, 0), 0),
    );
    assert_dates(
        &sol,
        w,
        want(at(5, 20, 0), at(5, 21, 0), at(5, 20, 0), at(5, 21, 0), 0),
    );
    let zero_float: Vec<_> = sol
        .dates
        .iter()
        .enumerate()
        .filter_map(|(i, dates)| dates.is_critical().then_some(TaskId(i as u32)))
        .collect();
    assert_eq!(zero_float, vec![x, y, w]);
    assert_eq!(s.driving_predecessors(&sol, y), vec![]);
    assert_eq!(s.driving_predecessors(&sol, w), vec![y]);
    // The zero-float set has X, but the longest path does not.
    assert_eq!(s.longest_path(&sol), vec![y, w]);
}

/// A(3) drives B(4) and C(4); both finish at hour 7 and drive D(5), which
/// finishes at hour 12 and drives E(1) to the hour-13 project finish.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn longest_path_keeps_both_equal_driver_chains() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let a = task(&mut s, cal, 3 * H, None);
    let b = task(&mut s, cal, 4 * H, None);
    let c = task(&mut s, cal, 4 * H, None);
    let d = task(&mut s, cal, 5 * H, None);
    let e = task(&mut s, cal, H, None);
    for (pred, succ) in [(a, b), (a, c), (b, d), (c, d), (d, e)] {
        dep(&mut s, pred, succ, FS, 0);
    }
    let sol = finish(&mut s);

    assert_eq!(sol.project_finish, 13 * H);
    assert_eq!(s.longest_path(&sol), vec![a, b, c, d, e]);
}

/// On a 24/7 calendar, independent P(5) and Q(5) both finish at the hour-5
/// project finish, while independent R(2) finishes at hour 2.
#[test]
fn longest_path_takes_every_finishing_task() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let p = task(&mut s, cal, 5 * H, None);
    let q = task(&mut s, cal, 5 * H, None);
    let _r = task(&mut s, cal, 2 * H, None);
    let sol = finish(&mut s);

    assert_eq!(sol.project_finish, 5 * H);
    assert_eq!(s.longest_path(&sol), vec![p, q]);
}

/// A solution with one task cannot describe the same schedule after a second
/// task is added, so it has no longest-path members for that schedule.
#[test]
fn longest_path_is_empty_for_a_foreign_solution() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    task(&mut s, cal, H, None);
    let sol = finish(&mut s);
    task(&mut s, cal, H, None);

    assert_eq!(s.longest_path(&sol), Vec::new());
}

/// With SS +120, A's Monday 08:00 start sets B's Monday 10:00 start; with
/// FS -120, B's Monday 15:00 finish sets C's Monday 13:00 start.
#[test]
fn mixed_ss_positive_fs_negative_driving_dependencies() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let a = task(&mut s, std, 600, None);
    let b = task(&mut s, six, 300, None);
    let c = task(&mut s, std, 240, None);
    dep(&mut s, a, b, SS, 120);
    dep(&mut s, b, c, FS, -120);
    let sol = finish(&mut s);

    assert_eq!(s.driving_predecessors(&sol, b), vec![a]);
    assert_eq!(s.driving_predecessors(&sol, c), vec![b]);
    assert_eq!(s.driven_successors(&sol, a), vec![b]);
    assert_eq!(s.driven_successors(&sol, b), vec![c]);
}

/// A starts Friday 13:00 from SNET. A's FF -60 bound makes B start Saturday
/// 15:00, and B's SF +30 bound makes C start Friday 15:30.
#[test]
fn mixed_ff_negative_sf_positive_snet_driving_dependencies() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let a = task(&mut s, std, 360, Some(at(4, 13, 0)));
    let b = task(&mut s, six, 180, Some(at(2, 8, 0)));
    let c = task(&mut s, std, 120, None);
    dep(&mut s, a, b, FF, -60);
    dep(&mut s, b, c, SF, 30);
    let sol = finish(&mut s);

    assert_eq!(s.driving_predecessors(&sol, a), vec![]);
    assert_eq!(s.driving_predecessors(&sol, b), vec![a]);
    assert_eq!(s.driving_predecessors(&sol, c), vec![b]);
}

/// The lag calendar fixture. P runs Mon-Fri 08:00-17:00 and lasts five days,
/// so it finishes Friday 17:00 (day 4). S runs Mon-Sat 08:00-17:00 and lasts
/// one hour. P -> S is FS with a 16 h (960 minute) lag. A third calendar,
/// every day 08:00-12:00, exists for `Fixed`. Returns (schedule, P, S).
fn weekend_lag(
    choice: impl FnOnce(engine::CalendarId) -> LagCalendar,
) -> (Schedule, TaskId, TaskId) {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let mornings = s.add_calendar(mornings_cal());
    let p = task(&mut s, std, 5 * 540, None);
    let q = task(&mut s, six, 60, None);
    dep(&mut s, p, q, FS, 960);
    s.set_lag_calendar(choice(mornings))
        .expect("the lag calendar is accepted");
    (s, p, q)
}

/// Solve `weekend_lag` and check P (Mon 08:00 - Fri 17:00, critical) and S
/// (starts at `s_start`, one hour, critical: it ends the project).
fn check_weekend_lag(choice: impl FnOnce(engine::CalendarId) -> LagCalendar, s_start: i64) {
    let (mut s, p, q) = weekend_lag(choice);
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, s_start + 60);
    assert_dates(
        &sol,
        p,
        want(at(0, 8, 0), at(4, 17, 0), at(0, 8, 0), at(4, 17, 0), 0),
    );
    assert_dates(
        &sol,
        q,
        want(s_start, s_start + 60, s_start, s_start + 60, 0),
    );
    assert_critical_chain(&s, &sol, &[p, q]);
}

/// Successor (Mon-Sat): from Fri 17:00, Saturday gives 540 minutes and Monday
/// the other 420, so the lag ends Mon 15:00 and S starts there.
#[test]
fn lag_calendar_successor() {
    check_weekend_lag(|_| LagCalendar::Successor, at(7, 15, 0));
}

/// Predecessor (Mon-Fri): Monday gives 540, Tuesday the other 420, so the lag
/// ends Tue 15:00, a working time of S's calendar.
#[test]
fn lag_calendar_predecessor() {
    check_weekend_lag(|_| LagCalendar::Predecessor, at(8, 15, 0));
}

/// `TwentyFourHour`: 16 elapsed hours from Fri 17:00 end Sat 09:00.
#[test]
fn lag_calendar_twenty_four_hour() {
    check_weekend_lag(|_| LagCalendar::TwentyFourHour, at(5, 9, 0));
}

/// Fixed to a calendar that works every day 08:00-12:00 (240 minutes a day):
/// Sat, Sun, Mon and Tue give 960, so the lag ends Tue 12:00.
#[test]
fn lag_calendar_fixed() {
    check_weekend_lag(LagCalendar::Fixed, at(8, 12, 0));
}

/// A lead on `TwentyFourHour`. P (Mon-Fri) takes two days, Mon 08:00 to Tue
/// 17:00 (instant 2460). S (Mon-Sat, one hour) follows by FS with `lag`. P
/// ends the project, so it has no float. S has no successor: it can slip to
/// Tue 16:00 (instant 2400).
fn lead_on_twenty_four_hour(lag: i64, s_start: i64, s_float: i64) {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let p = task(&mut s, std, 2 * 540, None);
    let q = task(&mut s, six, 60, None);
    dep(&mut s, p, q, FS, lag);
    s.set_lag_calendar(LagCalendar::TwentyFourHour)
        .expect("the lag calendar is accepted");
    let sol = finish(&mut s);
    assert_eq!(sol.project_finish, at(1, 17, 0));
    assert_dates(
        &sol,
        p,
        want(at(0, 8, 0), at(1, 17, 0), at(0, 8, 0), at(1, 17, 0), 0),
    );
    assert_dates(
        &sol,
        q,
        want(s_start, s_start + 60, at(1, 16, 0), at(1, 17, 0), s_float),
    );
}

/// The lead of 3000 minutes reaches before the origin from instant 2460
/// (2460 - 3000 < 0), so it clamps to the origin and S starts at the first
/// working minute of its calendar, Mon 08:00. S can slip from Mon 08:00 to
/// Tue 16:00: Monday gives 540 working minutes, Tuesday 08:00-16:00 480 more.
#[test]
fn lag_calendar_lead_clamps_at_the_origin() {
    lead_on_twenty_four_hour(-3000, at(0, 8, 0), 540 + 480);
}

/// A lead of 1500 minutes does not clamp on a 24-hour calendar: 2460 - 1500
/// is instant 960 (Mon 16:00), a working minute of S. Under the successor's
/// calendar the same lead would clamp (S has only 1080 working minutes before
/// 2460), so this date is what `TwentyFourHour` adds. Float: Mon 16:00-17:00
/// is 60 working minutes, Tue 08:00-16:00 is 480.
#[test]
fn lag_calendar_lead_that_does_not_clamp() {
    lead_on_twenty_four_hour(-1500, at(0, 16, 0), 60 + 480);
}
