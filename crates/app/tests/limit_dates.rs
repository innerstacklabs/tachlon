//! A date alone on a date limit is read on the task's calendar (decision 0017, rule 6), and "start on" and "finish on"
//! are a floor and a ceiling on one date: the bar is not moved by the limit and float can go negative. Every network
//! runs on a Monday-to-Friday calendar of 08:00 to 16:00 and starts on Monday 2026-01-05 (day 0).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use app::{Session, host::KeyTarget, meta::TaskMeta};
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const DAY: i64 = 1440;
/// A working day of the calendar, in minutes.
const WORK: i64 = 480;

/// The tasks `a` to `k` (keys `a`, `b`, `c`, `d`, `e`, `f`, `g`, `h`, `i`, `k`), one working day long apart from the five-day `a` and
/// `c`, with `a` then `b`, `c` then `d`, `e` then `f` and `g` then `h` linked.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let week = std::array::from_fn(|day| if day < 5 { vec![(480, 960)] } else { vec![] });
    let calendar = schedule.add_calendar(Calendar::new(week, &[]).unwrap());
    let task = |duration| Task {
        duration,
        calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    let lengths = [5, 1, 5, 1, 1, 1, 1, 1, 1, 1].map(|days| days * WORK);
    for duration in lengths {
        schedule.add_task(task(duration)).unwrap();
    }
    for (pred, succ) in [(0, 1), (2, 3), (4, 5), (6, 7)] {
        let (pred, succ) = (TaskId(pred), TaskId(succ));
        let kind = LinkType::FS;
        let link = Dependency {
            pred,
            succ,
            kind,
            lag: 0,
        };
        schedule.add_dependency(link).unwrap();
    }
    let solution = schedule.compute().unwrap();
    let order = (0..lengths.len() as u32).collect();
    let (timeline, meta) = (
        Timeline::new(order, 1_000.0, 400.0, 1.0),
        vec![TaskMeta::default(); 10],
    );
    let mut session = Session::new(schedule, solution, timeline, meta, || 0.0);
    for (at, key) in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "k"]
        .iter()
        .enumerate()
    {
        session
            .assign_key(KeyTarget::Task(TaskId(at as u32)), key)
            .unwrap();
    }
    session
}

/// Task `at`'s early start and finish, late start and finish, total float and free float, in minutes.
fn dates(session: &Session, at: usize) -> [i64; 6] {
    let d = session.solution().dates[at];
    let free = session.schedule().free_floats(session.solution()).unwrap();
    [d.es, d.ef, d.ls, d.lf, d.float, free[at]]
}

/// Monday of the week `week` (0 is 2026-01-05) at `hour` o'clock.
fn at(week: i64, day: i64, hour: i64) -> i64 {
    (week * 7 + day) * DAY + hour * 60
}

#[test]
fn a_start_on_and_a_finish_on_that_logic_pushes_past_keep_the_bar_and_go_negative() {
    let mut s = session();
    let both = r#"[{"key":"b","snet":"2026-01-07","snlt":"2026-01-07"},{"key":"d","fnet":"2026-01-07","fnlt":"2026-01-07"}]"#;
    s.update_tasks(both).unwrap();
    // Logic puts both on Monday 2026-01-12; the date, Wednesday, is neither floor nor pin for the bar.
    let b = [
        at(1, 0, 8),
        at(1, 0, 16),
        at(0, 2, 8),
        at(0, 2, 16),
        -3 * WORK,
        0,
    ];
    let d = [
        at(1, 0, 8),
        at(1, 0, 16),
        at(0, 2, 8),
        at(0, 2, 16),
        -3 * WORK,
        0,
    ];
    assert_eq!(dates(&s, 1), b);
    assert_eq!(dates(&s, 3), d);
}

#[test]
fn a_start_on_and_a_finish_on_after_the_logic_hold_the_bar_at_the_date() {
    let mut s = session();
    let both = r#"[{"key":"f","snet":"2026-01-14","snlt":"2026-01-14"},{"key":"h","fnet":"2026-01-14","fnlt":"2026-01-14"}]"#;
    s.update_tasks(both).unwrap();
    let wednesday = [at(1, 2, 8), at(1, 2, 16), at(1, 2, 8), at(1, 2, 16), 0, 0];
    assert_eq!(dates(&s, 5), wednesday);
    assert_eq!(dates(&s, 7), wednesday);
}

#[test]
fn a_date_alone_is_the_last_working_minute_for_a_finish_limit_and_the_first_for_a_start_limit() {
    let mut s = session();
    s.update_tasks(r#"[{"key":"i","fnet":"2026-01-07"},{"key":"k","snlt":"2026-01-07"}]"#)
        .unwrap();
    // The finish is 16:00 that Wednesday, not the next working day's; the late start is 08:00 that Wednesday.
    let finish = dates(&s, 8);
    assert_eq!((finish[0], finish[1]), (at(0, 2, 8), at(0, 2, 16)));
    let start = dates(&s, 9);
    assert_eq!(
        (start[0], start[2], start[4]),
        (at(0, 0, 8), at(0, 2, 8), 2 * WORK)
    );
}
