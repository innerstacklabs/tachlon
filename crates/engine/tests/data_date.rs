//! The data date (gs#703, story 1): work that has not started cannot start before it. The forward
//! pass floors every unstarted task's early start at the data date, as a start-no-earlier-than
//! would; a task's own SNET or a predecessor that lands later still wins. Every expected date
//! below was worked out by hand (calendar: Monday to Friday, 08:00 to 17:00; day 0 is the origin
//! Monday).

mod common;

use common::{assert_dates, at, check_invariants, dep, std_cal, task, want};
use engine::LinkType::FS;
use engine::{DurationCone, Schedule, TaskId};

/// One task of eight working hours on the Monday-to-Friday calendar.
fn lone_task() -> (Schedule, TaskId) {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, 8 * 60, None);
    (s, a)
}

/// No predecessor, data date Wednesday 08:00: the task starts Wednesday 08:00, not Monday. The
/// eight hours end Wednesday 16:00, the project finish; the task is the whole project, so its
/// float is 0.
#[test]
fn an_unstarted_task_starts_no_earlier_than_the_data_date() {
    let (mut s, a) = lone_task();
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    check_invariants(&s, &sol);
    assert_eq!(sol.get(a).es, at(2, 8, 0));
    assert_dates(
        &sol,
        a,
        want(at(2, 8, 0), at(2, 16, 0), at(2, 8, 0), at(2, 16, 0), 0),
    );
    assert_eq!(sol.project_finish, at(2, 16, 0));
}

/// A data date outside working time: Wednesday 17:30 is after the day ends, so the task starts at
/// the next admissible instant, Thursday 08:00.
#[test]
fn a_data_date_outside_working_time_starts_the_task_at_the_next_working_instant() {
    let (mut s, a) = lone_task();
    s.set_data_date(Some(at(2, 17, 30)));
    let sol = s.compute().expect("computes");
    check_invariants(&s, &sol);
    assert_dates(
        &sol,
        a,
        want(at(3, 8, 0), at(3, 16, 0), at(3, 8, 0), at(3, 16, 0), 0),
    );
}

/// The data date is a floor, not a shift. A has SNET Thursday 08:00 and B follows A: both start
/// where their own bounds say, later than a Wednesday data date, and C, with nothing before it,
/// starts at the data date.
#[test]
fn a_later_bound_beats_the_data_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, 8 * 60, Some(at(3, 8, 0)));
    let b = task(&mut s, c, 8 * 60, None);
    let free = task(&mut s, c, 8 * 60, None);
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    let sol = s.compute().expect("computes");
    check_invariants(&s, &sol);
    // A: Thursday 08:00 to 16:00. B: from A's finish, Thursday 16:00, eight hours (one left on
    // Thursday, seven on Friday) to Friday 15:00.
    assert_eq!(sol.get(a).es, at(3, 8, 0));
    assert_eq!(sol.get(b).es, at(3, 16, 0));
    assert_eq!(sol.get(b).ef, at(4, 15, 0));
    // C starts at the data date.
    assert_eq!(sol.get(free).es, at(2, 8, 0));
}

/// Clearing the data date gives back the dates of a schedule that never had one.
#[test]
fn clearing_the_data_date_restores_the_origin() {
    let (mut s, a) = lone_task();
    let before = s.compute().expect("computes");
    assert_eq!(before.get(a).es, at(0, 8, 0));
    s.set_data_date(Some(at(2, 8, 0)));
    assert_eq!(s.data_date(), Some(at(2, 8, 0)));
    assert_eq!(s.compute().expect("computes").get(a).es, at(2, 8, 0));
    s.set_data_date(None);
    assert_eq!(s.data_date(), None);
    assert_eq!(s.compute().expect("computes"), before);
}

/// A duration edit solved by the duration cone keeps the data date. A (eight hours) -> B, and a
/// separate five-day task that sets the project finish, so shortening A moves no finish and the
/// cone solves the edit itself instead of falling back to a full compute (thirty small tasks
/// raise its limit). The data date is Wednesday 08:00; A must stay there.
#[test]
fn the_duration_cone_keeps_the_data_date() {
    let mut s = Schedule::new();
    let c = s.add_calendar(std_cal(&[]));
    let a = task(&mut s, c, 8 * 60, None);
    let b = task(&mut s, c, 8 * 60, None);
    let _long = task(&mut s, c, 5 * 9 * 60, None);
    // Enough tasks that the cone may admit A and B (its limit is a tenth of the live tasks).
    for _ in 0..30 {
        task(&mut s, c, 60, None);
    }
    dep(&mut s, a, b, FS, 0);
    s.set_data_date(Some(at(2, 8, 0)));
    let mut sol = s.compute().expect("computes");
    s.set_duration(a, 6 * 60).expect("task exists");
    let mut moved = Vec::new();
    let how = s
        .recompute_duration_cone_moved(&mut sol, a, &mut moved)
        .expect("solves");
    assert_eq!(how, DurationCone::Cone);
    assert_eq!(sol, s.compute().expect("computes"));
    assert_eq!(sol.get(a).es, at(2, 8, 0));
    assert_eq!(sol.get(a).ef, at(2, 14, 0));
}
