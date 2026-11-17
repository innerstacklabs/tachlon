//! astra's two counterexamples from plan review, as named fixtures. Both
//! break an engine that inverts `add` by negating a lag or subtracting a
//! duration. std = Mon-Fri 08:00-17:00; six = Mon-Sat 08:00-17:00. Day 4 is
//! Friday, day 5 Saturday, day 7 the next Monday.

mod common;

use common::{assert_dates, at, check_invariants, dep, oracle, six_cal, std_cal, task, want};
use engine::LinkType::{FF, FS};
use engine::Schedule;

/// (a) FF, zero lag. Successor S: std, 60 min. Predecessor P: six, finishing
/// Saturday 12:00 (240 min from a Saturday 08:00 SNET).
///
/// es(S) is the earliest working s with fin(S, s) >= Sat 12:00:
/// Fri 16:00 gives Fri 17:00 (too early); Fri 16:01 gives 59 minutes Friday
/// plus 1 Monday = Mon 08:01. So es(S) = Fri 16:01, not Fri 16:00.
///
/// Backward, PF = Mon 08:01: S ls Fri 16:01. P: fin(P, x) <= Mon 08:01
/// -> x = Sat 13:01 (239 Sat + 1 Mon); float 301 (Sat 08:00 to 13:01).
#[test]
fn counterexample_a_ff_zero_lag_into_weekday_calendar() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let p = task(&mut s, six, 240, Some(at(5, 8, 0)));
    let succ = task(&mut s, std, 60, None);
    dep(&mut s, p, succ, FF, 0);
    let sol = s.compute().expect("computes");
    check_invariants(&s, &sol);
    assert_eq!(sol.get(p).ef, at(5, 12, 0), "P finishes Saturday 12:00");
    assert_ne!(
        sol.get(succ).es,
        at(4, 16, 0),
        "the bug: es(S) = Friday 16:00"
    );
    assert_eq!(sol.get(succ).es, at(4, 16, 1));
    assert_eq!(sol.get(succ).ef, at(7, 8, 1));
    assert_eq!(sol.project_finish, at(7, 8, 1));
    assert_dates(
        &sol,
        succ,
        want(at(4, 16, 1), at(7, 8, 1), at(4, 16, 1), at(7, 8, 1), 0),
    );
    assert_dates(
        &sol,
        p,
        want(at(5, 8, 0), at(5, 12, 0), at(5, 13, 1), at(7, 8, 1), 301),
    );
    oracle::assert_matches(&mut s, "counterexample a");
}

/// (b) FS, lag -60. Successor S: std, 60 min, with ls(S) = Fri 16:00.
/// Predecessor P: six, finishing Saturday 12:00.
///
/// Backward for P: add(std, fin(P, x), -60) <= Fri 16:00. At x = Sat 08:00,
/// fin = Sat 12:00 and add(std, Sat 12:00, -60) = Fri 16:00, which holds. So a
/// Saturday 12:00 finish stays feasible. (Inverting instead, lf(P) <=
/// ls(S) + 60 = Fri 17:00, would make P infeasible.)
#[test]
fn counterexample_b_fs_negative_lag_backward_feasible() {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    let p = task(&mut s, six, 240, Some(at(5, 8, 0)));
    let succ = task(&mut s, std, 60, None);
    dep(&mut s, p, succ, FS, -60);
    let sol = s
        .compute()
        .expect("a Saturday 12:00 predecessor finish is feasible");
    check_invariants(&s, &sol);
    assert_eq!(sol.project_finish, at(5, 12, 0));
    assert_dates(
        &sol,
        succ,
        want(at(4, 16, 0), at(4, 17, 0), at(4, 16, 0), at(4, 17, 0), 0),
    );
    assert_eq!(sol.get(succ).ls, at(4, 16, 0), "ls(S) = Friday 16:00");
    assert_dates(
        &sol,
        p,
        want(at(5, 8, 0), at(5, 12, 0), at(5, 8, 0), at(5, 12, 0), 0),
    );
    assert_eq!(
        sol.get(p).lf,
        at(5, 12, 0),
        "predecessor finish Saturday 12:00 is feasible"
    );
    oracle::assert_matches(&mut s, "counterexample b");
}
