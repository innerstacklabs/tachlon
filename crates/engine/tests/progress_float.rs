//! Float once work is under way, one hand-solved case per rule of decision 0008
//! (docs/decisions/0008-progress-model.md) that `progress.rs` leaves without a float check.
//! Calendar: Monday to Friday, 08:00 to 17:00, nine working hours a day; day 0 is the origin
//! Monday, so day 3 is Thursday, day 7 the next Monday.

mod common;

use common::{at, dep, std_cal, task};
use engine::LinkType::FS;
use engine::Schedule;

/// Minutes in `n` working days of the test calendar.
fn days(n: i64) -> i64 {
    n * 9 * 60
}

/// Pick 3, and a finished successor binds nobody. P (one day, unstarted) precedes F finish-to-start;
/// F is finished Monday 08:00 to 17:00. L (five days, unstarted) runs alongside and sets the
/// project finish. The data date is Thursday 08:00, so P and L start there; L ends on Wednesday of
/// the second week (day 9) at 17:00, the project finish.
///
/// By hand: F stays at its actuals with float 0. F binds nothing, so P may run as late as the
/// project finish allows: late start Wednesday (day 9) 08:00, late finish 17:00. P's float is the
/// working time from its early start, Thursday 08:00, to its late start: Thursday, Friday, Monday,
/// Tuesday, four days. L has no float and is critical.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn a_finished_task_has_no_float_and_a_finished_successor_leaves_its_predecessor_slack() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = task(&mut s, c, days(1), None);
    let f = task(&mut s, c, days(1), None);
    let l = task(&mut s, c, days(5), None);
    s.set_actual_start(f, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(f, Some(at(0, 17, 0))).expect("exists");
    dep(&mut s, p, f, FS, 0);
    s.set_data_date(Some(at(3, 8, 0))).expect("in range");
    let sol = s.compute().expect("computes");
    assert_eq!(sol.project_finish, at(9, 17, 0));
    let df = sol.get(f);
    assert_eq!((df.es, df.ef), (at(0, 8, 0), at(0, 17, 0)));
    assert_eq!((df.ls, df.lf, df.float), (at(0, 8, 0), at(0, 17, 0), 0));
    assert!(
        !df.is_critical(),
        "a finished task is never late, and not critical"
    );
    let dp = sol.get(p);
    assert_eq!((dp.es, dp.ef), (at(3, 8, 0), at(3, 17, 0)));
    assert_eq!((dp.ls, dp.lf), (at(9, 8, 0), at(9, 17, 0)));
    assert_eq!(dp.float, days(4));
    assert!(!dp.is_critical());
    let dl = sol.get(l);
    assert_eq!((dl.es, dl.ef), (at(3, 8, 0), at(9, 17, 0)));
    assert_eq!((dl.ls, dl.lf, dl.float), (at(3, 8, 0), at(9, 17, 0), 0));
}

/// Pick 2 and 4: the float of a started task whose successor is finish-to-start. A is five planned
/// days, started Monday 08:00, two days remaining; the data date is Wednesday 08:00. B (one day)
/// follows A finish-to-start. L (five days, unstarted) sets the project finish.
///
/// By hand: A's remaining days run Wednesday and Thursday, early finish Thursday 17:00 (day 3). B
/// runs Friday (day 4). L runs Wednesday to Tuesday of the second week and ends day 8 at 17:00,
/// the project finish. B's late start is Tuesday 08:00 (day 8), so B's float, early start Friday
/// 08:00 to late start Tuesday 08:00, is Friday and Monday: two days. A must finish before B's
/// late start: its late finish is Monday (day 7) 17:00, its late start (two days) Friday 08:00. A's
/// float is measured from its early finish, Thursday 17:00, to its late finish, Monday 17:00:
/// Friday and Monday, two days, the same as B, which is how long the chain can slip. Measured from
/// the actual start it would count the days already worked and say four.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn a_started_task_has_float_from_its_early_finish() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(5), None);
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_remaining(a, Some(days(2))).expect("exists");
    let b = task(&mut s, c, days(1), None);
    let l = task(&mut s, c, days(5), None);
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(2, 8, 0))).expect("in range");
    let sol = s.compute().expect("computes");
    assert_eq!(sol.project_finish, at(8, 17, 0));
    let (da, db, dl) = (sol.get(a), sol.get(b), sol.get(l));
    assert_eq!((da.es, da.ef), (at(0, 8, 0), at(3, 17, 0)));
    assert_eq!((db.es, db.ef), (at(4, 8, 0), at(4, 17, 0)));
    assert_eq!((dl.es, dl.ef), (at(2, 8, 0), at(8, 17, 0)));
    assert_eq!(
        (db.ls, db.lf, db.float),
        (at(8, 8, 0), at(8, 17, 0), days(2))
    );
    assert_eq!((da.ls, da.lf), (at(4, 8, 0), at(7, 17, 0)));
    assert_eq!(da.float, days(2));
    assert_eq!(dl.float, 0);
}

/// The same schedule, pinning the late dates of A from its remaining work.
#[test]
fn a_started_task_has_late_dates_from_its_remaining_work_before_a_finish_to_start_successor() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(5), None);
    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_remaining(a, Some(days(2))).expect("exists");
    let b = task(&mut s, c, days(1), None);
    let _l = task(&mut s, c, days(5), None);
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(2, 8, 0))).expect("in range");
    let sol = s.compute().expect("computes");
    let (da, db) = (sol.get(a), sol.get(b));
    assert_eq!((da.ls, da.lf), (at(4, 8, 0), at(7, 17, 0)));
    assert_eq!(
        (db.es, db.ls, db.float),
        (at(4, 8, 0), at(8, 8, 0), days(2))
    );
    assert_eq!(da.float, days(2));
}
