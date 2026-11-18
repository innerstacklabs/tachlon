//! `engine::diff` with progress (gs#703): finishing a task takes it off the critical path, and
//! the diff reports that (decision 0008, point 3a: a finished task is never critical).
//! Calendar: Monday to Friday, 08:00 to 17:00, nine working hours a day.

mod common;

use common::{at, dep, std_cal, task};
use engine::LinkType::FS;
use engine::{Schedule, diff};

/// Minutes in `n` working days of the test calendar.
fn days(n: i64) -> i64 {
    n * 9 * 60
}

/// A lone one-day task has zero float and is critical. Starting and finishing it, with the data
/// date after, leaves it at zero float but finished, so it leaves the critical path. Clearing the
/// actual finish and diffing back puts it on the path again.
#[test]
fn finishing_the_only_task_leaves_the_critical_path_and_reopening_it_rejoins() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let t = task(&mut s, c, days(1), None);
    let before = s.compute().expect("computes");
    assert!(before.get(t).is_critical(), "a lone task is critical");

    s.set_actual_start(t, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(t, Some(at(0, 17, 0))).expect("exists");
    s.set_data_date(Some(at(1, 8, 0))).expect("in range");
    let finished = s.compute().expect("computes");
    assert_eq!(finished.get(t).float, 0);
    assert!(
        !finished.get(t).is_critical(),
        "a finished task is not critical"
    );

    let went = diff(&before, &finished);
    assert_eq!(went.left_critical, vec![t]);
    assert_eq!(went.joined_critical, Vec::new());

    s.set_actual_finish(t, None).expect("exists");
    let reopened = s.compute().expect("computes");
    assert!(
        reopened.get(t).is_critical(),
        "an unfinished task is critical again"
    );

    let back = diff(&finished, &reopened);
    assert_eq!(back.joined_critical, vec![t]);
    assert_eq!(back.left_critical, Vec::new());
}

/// P (one day) runs alongside L (five days), so P has float and is not critical. Finishing P
/// changes neither list: it was off the path and stays off it, and L stays on it.
#[test]
fn finishing_a_task_that_was_never_critical_changes_neither_list() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let p = task(&mut s, c, days(1), None);
    let l = task(&mut s, c, days(5), None);
    let before = s.compute().expect("computes");
    assert!(!before.get(p).is_critical(), "P has float");
    assert!(before.get(l).is_critical());

    s.set_actual_start(p, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(p, Some(at(0, 17, 0))).expect("exists");
    s.set_data_date(Some(at(1, 8, 0))).expect("in range");
    let after = s.compute().expect("computes");
    assert!(!after.get(p).is_critical());
    assert!(after.get(l).is_critical());

    let got = diff(&before, &after);
    assert_eq!(got.joined_critical, Vec::new());
    assert_eq!(got.left_critical, Vec::new());
}

/// A chain does not matter to the rule: finishing the first of two linked tasks takes it off the
/// path and leaves the second on it.
#[test]
fn finishing_the_head_of_a_chain_drops_only_the_head() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, days(1), None);
    let b = task(&mut s, c, days(1), None);
    dep(&mut s, a, b, FS, 0);
    let before = s.compute().expect("computes");

    s.set_actual_start(a, Some(at(0, 8, 0))).expect("exists");
    s.set_actual_finish(a, Some(at(0, 17, 0))).expect("exists");
    s.set_data_date(Some(at(1, 8, 0))).expect("in range");
    let after = s.compute().expect("computes");

    let got = diff(&before, &after);
    assert_eq!(got.left_critical, vec![a]);
    assert_eq!(got.joined_critical, Vec::new());
}
