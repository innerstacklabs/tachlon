//! Free float, `Schedule::free_floats`, worked by hand on a Monday-to-Friday 08:00-17:00 calendar
//! (540 working minutes a day, day 0 the origin Monday), then checked against the forward pass.

mod common;

use common::{at, cont_cal, dep, std_cal, task};
use engine::LinkType::{self, FF, FS, SF, SS};
use engine::{CalendarId, Instant, Limit, Minutes, Schedule, ScheduleError, Solution, TaskId};

const D: Minutes = 540;
const DAY: (Minutes, Option<Instant>) = (D, None);

/// A week-calendar schedule: tasks (duration, start-no-earlier-than), links (pred, succ, kind, lag).
fn net(
    tasks: &[(Minutes, Option<Instant>)],
    links: &[(usize, usize, LinkType, Minutes)],
) -> Schedule {
    let mut s = Schedule::new();
    let cal = s.add_calendar(std_cal(&[]));
    let ids: Vec<_> = tasks
        .iter()
        .map(|&(len, snet)| task(&mut s, cal, len, snet))
        .collect();
    for &(p, q, kind, lag) in links {
        dep(&mut s, ids[p], ids[q], kind, lag);
    }
    s
}

/// Free and total float of every task.
fn floats(s: &mut Schedule) -> (Vec<Minutes>, Vec<Minutes>) {
    let sol = s.compute().expect("computes");
    (
        s.free_floats(&sol).expect("free floats"),
        sol.dates.iter().map(|d| d.float).collect(),
    )
}

/// Tasks 0 to 4: `first`, `mid`, `last` in a chain of days, `long` (three days) also before
/// `last`, and `five` days, which sets the project finish on Friday.
fn slack() -> Schedule {
    let tasks = [DAY, DAY, DAY, (3 * D, None), (5 * D, None)];
    net(&tasks, &[(0, 1, FS, 0), (1, 2, FS, 0), (3, 2, FS, 0)])
}

/// P (two days from Monday) linked to Q, which starts no earlier than `q_start`, with a lag of
/// one day, beside a ten-day task: P's free and total float.
fn linked(kind: LinkType, q_start: Instant) -> (Minutes, Minutes) {
    let tasks = [(2 * D, None), (D, Some(q_start)), (10 * D, None)];
    let (free, total) = floats(&mut net(&tasks, &[(0, 1, kind, D)]));
    (free[0], total[0])
}

/// Diamond: A, then B (a day) and C (three days), then E. B can end with C on Wednesday, before E
/// starts on Thursday: two days of free float, also its total float.
#[test]
fn a_diamond_gives_its_short_branch_free_float_equal_to_its_total() {
    let links = [(0, 1, FS, 0), (0, 2, FS, 0), (1, 3, FS, 0), (2, 3, FS, 0)];
    let mut s = net(&[DAY, DAY, (3 * D, None), DAY], &links);
    assert_eq!(floats(&mut s), (vec![0, 2 * D, 0, 0], vec![0, 2 * D, 0, 0]));
}

/// The last task has no successor: it runs free to the project finish, Friday 17:00.
#[test]
fn the_last_task_runs_free_to_the_project_finish() {
    assert_eq!(floats(&mut net(&[DAY, (5 * D, None)], &[])).0, [4 * D, 0]);
}

/// `last` waits for `long`: `mid` has a day of free float and two of total.
#[test]
fn free_float_is_the_room_before_the_next_task_not_the_project() {
    assert_eq!(
        floats(&mut slack()),
        (vec![0, D, D, 0, 0], vec![2 * D, 2 * D, D, D, 0])
    );
}

/// Each link counts its lag of a day from its own end. SS: Q starts Thursday, so P can start by
/// Wednesday. FF: Q ends Friday, so P can end Thursday. SF: Q ends Friday and cannot end before P
/// starts and a day has passed, so P can start by Friday 08:00.
#[test]
fn a_lag_counts_from_the_start_or_the_finish_by_link_type() {
    assert_eq!(linked(SS, at(3, 8, 0)).0, 2 * D);
    assert_eq!(linked(FF, at(4, 8, 0)).0, 2 * D);
    let (free, total) = linked(SF, at(4, 8, 0));
    assert_eq!((free, total > free), (4 * D, true));
}

/// A started successor is read at its remaining work's start, not its actual start: S began
/// Monday and resumes the next Monday; P, unstarted, starts at the data date (Wednesday) and can
/// end Friday 17:00 before S picks up.
#[test]
fn a_started_successor_is_read_at_its_remaining_start() {
    let mut s = net(&[DAY, (3 * D, None)], &[(0, 1, FS, 0)]);
    s.set_actual_start(TaskId(1), Some(at(0, 8, 0))).unwrap();
    s.set_remaining(TaskId(1), Some(2 * D)).unwrap();
    s.set_limit(
        TaskId(1),
        Limit {
            resume: Some(at(7, 8, 0)),
            ..Limit::default()
        },
    )
    .unwrap();
    s.set_data_date(Some(at(2, 8, 0))).expect("in range");
    assert_eq!(floats(&mut s).0[0], 2 * D);
}

/// A finished task has none and a finished successor binds nobody: P, which F follows, runs free
/// to the project finish, Wednesday of the second week.
#[test]
fn a_finished_task_has_none_and_a_finished_successor_takes_none() {
    let mut s = net(&[DAY, DAY, (5 * D, None)], &[(0, 1, FS, 0)]);
    s.set_actual_start(TaskId(1), Some(at(0, 8, 0))).unwrap();
    s.set_actual_finish(TaskId(1), Some(at(0, 17, 0))).unwrap();
    s.set_data_date(Some(at(3, 8, 0))).expect("in range");
    assert_eq!(floats(&mut s).0, [4 * D, 0, 0]);
}

/// An as-late-as-possible `mid` has none, and `first` gains the day it left; a finish limit
/// before `mid`'s early finish gives total float -D and still the day before `last` starts.
#[test]
fn an_as_late_as_possible_task_has_none_and_a_finish_limit_does_not_cap_it() {
    let mut s = slack();
    s.set_limit(
        TaskId(1),
        Limit {
            alap: true,
            ..Limit::default()
        },
    )
    .unwrap();
    assert_eq!(floats(&mut s).0, [D, 0, D, 0, 0]);
    s.set_limit(
        TaskId(1),
        Limit {
            fnlt: Some(at(0, 17, 0)),
            ..Limit::default()
        },
    )
    .unwrap();
    let (free, total) = floats(&mut s);
    assert_eq!((free[1], total[1]), (D, -D));
}

/// A must-finish-by date shortens the total float of Z (a day beside a chain of three that ends
/// Wednesday) but never its free float, which reads to the computed finish.
#[test]
fn a_must_finish_by_date_moves_total_float_and_not_free_float() {
    let mut s = net(&[DAY; 4], &[(1, 2, FS, 0), (2, 3, FS, 0)]);
    for (date, total_z) in [
        (None, 2 * D),
        (Some(at(1, 17, 0)), D),
        (Some(at(9, 17, 0)), 2 * D),
    ] {
        s.set_finish_by(date);
        let (free, total) = floats(&mut s);
        assert_eq!(
            (free[0], total[0], free[3]),
            (2 * D, total_z, 0),
            "{date:?}"
        );
    }
}

/// D (4 days) then E (3 days) beside an unlinked task F of 12 days: E has no successor and runs
/// days 6 and 7 of 12, so its free float is 5 days under every limit of its own (gantt#2127).
#[test]
fn a_tasks_own_late_limit_does_not_cap_its_free_float() {
    let (day5, day9) = (at(4, 17, 0), at(10, 17, 0));
    let limit = |fnet, fnlt| Limit {
        fnet,
        fnlt,
        ..Limit::default()
    };
    for (limit, total_e) in [
        (limit(None, None), 5 * D),
        (limit(None, Some(day9)), 2 * D),
        (limit(None, Some(day5)), -2 * D),
        (limit(Some(day5), Some(day5)), -2 * D),
    ] {
        let mut s = net(
            &[(4 * D, None), (3 * D, None), (12 * D, None)],
            &[(0, 1, FS, 0)],
        );
        s.set_limit(TaskId(1), limit).unwrap();
        let (free, total) = floats(&mut s);
        assert_eq!((free, total[1]), (vec![0, 5 * D, 0], total_e), "{limit:?}");
    }
}

/// Free float counts the task's own calendar: A (Monday to Friday) before B (24 hours, from
/// Sunday 00:00) can end Friday 17:00: four of A's days, not four 24-hour days.
#[test]
fn free_float_is_counted_on_the_tasks_own_calendar() {
    let mut s = Schedule::new();
    let (std, all) = (s.add_calendar(std_cal(&[])), s.add_calendar(cont_cal()));
    let (a, b) = (
        task(&mut s, std, D, None),
        task(&mut s, all, 1440, Some(at(6, 0, 0))),
    );
    dep(&mut s, a, b, FS, 0);
    assert_eq!(floats(&mut s).0[0], 4 * D);
}

/// A deleted slot has 0 even where its dates read as a live task's, an empty schedule gives no
/// values, and a solution with another task count is refused.
#[test]
fn a_deleted_slot_is_zero_and_a_solution_of_another_length_is_refused() {
    let mut empty = net(&[], &[]);
    let none = empty.compute().unwrap();
    assert_eq!(empty.free_floats(&none), Ok(vec![]));
    let mut s = net(&[DAY, (5 * D, None)], &[]);
    let before = s.compute().unwrap();
    assert_eq!(s.free_floats(&before).unwrap()[0], 4 * D);
    s.delete_task(TaskId(0)).unwrap();
    assert_eq!(s.free_floats(&before), Ok(vec![0, 0]));
    task(&mut s, CalendarId(0), D, None);
    assert_eq!(
        s.free_floats(&before),
        Err(ScheduleError::UnknownTask(TaskId(2)))
    );
}

/// Second opinion, from the forward pass: on a 24-hour calendar, delaying a task's start by its
/// free float (a start limit) moves no other early date nor the project finish, and one minute
/// more moves one. Four link types, with lags.
#[test]
fn delaying_a_task_by_its_free_float_moves_nothing_and_one_minute_more_does() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(cont_cal());
    let ids = [100, 60, 200, 50, 90, 30, 120].map(|len| task(&mut s, cal, len, None));
    for (p, q, kind, lag) in [
        (0, 1, FS, 10),
        (0, 2, SS, 30),
        (1, 3, FF, 20),
        (2, 3, FS, 0),
    ]
    .into_iter()
    .chain([(2, 4, SF, 150), (3, 5, FS, 0), (4, 5, FF, 0), (1, 6, SS, 0)])
    {
        dep(&mut s, ids[p], ids[q], kind, lag);
    }
    let sol = s.compute().unwrap();
    let free = s.free_floats(&sol).unwrap();
    let early = |sol: &Solution, skip: usize| {
        let others = sol.dates.iter().enumerate().filter(|&(j, _)| j != skip);
        (
            others.map(|(_, d)| (d.es, d.ef)).collect::<Vec<_>>(),
            sol.project_finish,
        )
    };
    for (i, id) in ids.iter().enumerate() {
        let moved_by = |extra: Minutes| {
            let mut late = s.clone();
            late.set_snet(*id, Some(sol.dates[i].es + free[i] + extra))
                .unwrap();
            early(&late.compute().unwrap(), i) != early(&sol, i)
        };
        assert!(!moved_by(0) && moved_by(1), "{id}: free float {}", free[i]);
    }
}
