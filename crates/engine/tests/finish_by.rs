//! The project must-finish-by date and the buffer (gantt#2074), worked by hand: the date missed by
//! two days, the date beaten by five, the calendar that measures the buffer (decision 1 of
//! devteam-1-1490), and the cones against a full compute.

mod common;

use common::{at, dep, mornings_cal, oracle, six_cal, std_cal, task};
use engine::{Buffer, CalendarId, DurationCone, LinkType, Schedule, TaskId};

const D: i64 = 540;

fn buf(minutes: i64, days: i64) -> Buffer {
    Buffer { minutes, days }
}

/// Three one-day tasks in a row on `cal`, each `len` working minutes.
fn three(s: &mut Schedule, cal: CalendarId, len: i64) -> [TaskId; 3] {
    let t = [0; 3].map(|_| task(s, cal, len, None));
    dep(s, t[0], t[1], LinkType::FS, 0);
    dep(s, t[1], t[2], LinkType::FS, 0);
    t
}

#[test]
fn a_date_missed_by_two_days_gives_every_task_on_the_path_minus_two_days() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    let t = three(&mut s, cal, D);
    // The chain finishes Wednesday 17:00; the date is Monday 17:00.
    s.set_finish_by(Some(at(0, 17, 0))).expect("in range");
    let sol = s.compute().expect("a missed date is not an error");
    assert_eq!(sol.project_finish, at(2, 17, 0));
    for id in t {
        assert_eq!(sol.get(id).float, -2 * D, "{id}");
        assert!(sol.get(id).is_critical());
    }
    assert_eq!(s.buffer(&sol), Some(buf(-2 * D, -2)));
    oracle::assert_matches(&mut s, "date missed by two days");
}

#[test]
fn a_date_beaten_by_five_days_keeps_the_critical_path_and_reports_the_buffer() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(mornings_cal());
    let t = three(&mut s, cal, 240);
    // The chain finishes Wednesday 12:00; the date is the next Monday 12:00.
    s.set_finish_by(Some(at(7, 12, 0))).expect("in range");
    let sol = s.compute().expect("solves");
    for id in t {
        assert_eq!(sol.get(id).float, 0, "{id}");
    }
    assert_eq!(s.buffer(&sol), Some(buf(5 * 240, 5)));
    oracle::assert_matches(&mut s, "date beaten by five days");
}

#[test]
fn the_buffer_is_measured_on_the_calendar_of_the_task_that_finishes_last() {
    let mut s = Schedule::new();
    let week = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    task(&mut s, week, 2 * D, None);
    let last = task(&mut s, six, 5 * D, None);
    // The six-day task finishes Friday 17:00; the date is the next Monday 17:00: Saturday and
    // Monday on its calendar, Monday alone on the first.
    s.set_finish_by(Some(at(7, 17, 0))).expect("in range");
    let sol = s.compute().expect("solves");
    assert_eq!(sol.get(last).ef, at(4, 17, 0));
    assert_eq!(s.buffer(&sol), Some(buf(2 * D, 3)));

    // Two six-day tasks finish last together: the schedule's first calendar measures it.
    task(&mut s, six, 5 * D, None);
    let sol = s.compute().expect("solves");
    assert_eq!(s.buffer(&sol), Some(buf(D, 3)));
    s.set_finish_by(None).expect("in range");
    assert_eq!(s.buffer(&sol), None);
}

#[test]
fn the_cones_with_a_date_set_equal_a_full_compute() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    // `a` then `b`, beside a five-day `c` that ends the project Friday; the date is Thursday.
    let (a, b) = (task(&mut s, cal, D, None), task(&mut s, cal, D, None));
    let c = task(&mut s, cal, 5 * D, None);
    dep(&mut s, a, b, LinkType::FS, 0);
    // Ten milestones, so the duration cone may admit `a` and `b` (a tenth of the tasks).
    for _ in 0..10 {
        task(&mut s, cal, 0, None);
    }
    s.set_finish_by(Some(at(3, 17, 0))).expect("in range");
    let mut sol = s.compute().expect("solves");

    // A longer `b` keeps the project finish and a positive float: the duration cone solves it,
    // and `b` itself is bound by the date.
    s.set_duration(b, 2 * D).expect("b exists");
    let mut moved = Vec::new();
    let how = s.recompute_duration_cone_moved(&mut sol, b, &mut moved);
    assert_eq!(how, Ok(DurationCone::Cone));
    assert_eq!(sol, s.compute().expect("solves"), "duration cone");
    assert_eq!(sol.get(b).float, D);

    // `c` pushed a day later moves the project finish: the drag cone runs the backward pass.
    s.set_snet(c, Some(at(1, 8, 0))).expect("c exists");
    s.recompute_cone(&mut sol, c).expect("solves");
    assert_eq!(sol, s.compute().expect("solves"), "drag cone");
    oracle::assert_matches(&mut s, "cones");
}
