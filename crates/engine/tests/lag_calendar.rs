//! The lag calendar choice (`Schedule::set_lag_calendar`): the setting's
//! contract, and the driving relationships that read it through
//! `dependency_start`.

mod common;

use common::{at, check_invariants, dep, mornings_cal, six_cal, std_cal, task};
use engine::LinkType::FS;
use engine::{CalendarId, LagCalendar, Schedule, ScheduleError, TaskId};

/// P (Mon-Fri, five days, ends Fri 17:00) -> S (Mon-Sat, one hour) by FS with
/// a 16 h lag, as in `hand_solved.rs`. Returns the schedule, P, S and the
/// start the choice gives S.
fn weekend_lag(lag: LagCalendar) -> (Schedule, TaskId, TaskId, i64) {
    let mut s = Schedule::new();
    let std = s.add_calendar(std_cal(&[]));
    let six = s.add_calendar(six_cal());
    s.add_calendar(mornings_cal());
    let p = task(&mut s, std, 5 * 540, None);
    let q = task(&mut s, six, 60, None);
    dep(&mut s, p, q, FS, 960);
    s.set_lag_calendar(lag)
        .expect("the lag calendar is accepted");
    let start = match lag {
        LagCalendar::Successor => at(7, 15, 0),
        LagCalendar::Predecessor => at(8, 15, 0),
        LagCalendar::TwentyFourHour => at(5, 9, 0),
        LagCalendar::Fixed(_) => at(8, 12, 0),
    };
    (s, p, q, start)
}

const CHOICES: [LagCalendar; 4] = [
    LagCalendar::Successor,
    LagCalendar::Predecessor,
    LagCalendar::TwentyFourHour,
    LagCalendar::Fixed(CalendarId(2)),
];

#[test]
fn the_default_is_the_successors_calendar() {
    assert_eq!(Schedule::new().lag_calendar(), LagCalendar::Successor);
    assert_eq!(LagCalendar::default(), LagCalendar::Successor);
}

#[test]
fn a_set_choice_is_read_back() {
    for lag in CHOICES {
        let (s, ..) = weekend_lag(lag);
        assert_eq!(s.lag_calendar(), lag);
    }
}

#[test]
fn an_unknown_fixed_calendar_is_refused_and_changes_nothing() {
    let (mut s, p, q, start) = weekend_lag(LagCalendar::Predecessor);
    for id in [CalendarId(3), CalendarId(u32::MAX)] {
        assert_eq!(
            s.set_lag_calendar(LagCalendar::Fixed(id)),
            Err(ScheduleError::UnknownCalendar(id))
        );
        assert_eq!(s.lag_calendar(), LagCalendar::Predecessor);
    }
    let sol = s.compute().unwrap();
    assert_eq!(sol.get(q).es, start);
    assert_eq!(sol.get(p).es, at(0, 8, 0));
    // The last calendar is accepted: the id one past it is the first refused.
    s.set_lag_calendar(LagCalendar::Fixed(CalendarId(2)))
        .unwrap();
}

#[test]
fn an_empty_schedule_accepts_every_choice_but_an_unknown_calendar() {
    let mut s = Schedule::new();
    for lag in [
        LagCalendar::Successor,
        LagCalendar::Predecessor,
        LagCalendar::TwentyFourHour,
    ] {
        s.set_lag_calendar(lag).unwrap();
        assert_eq!(s.lag_calendar(), lag);
    }
    assert_eq!(
        s.set_lag_calendar(LagCalendar::Fixed(CalendarId(0))),
        Err(ScheduleError::UnknownCalendar(CalendarId(0)))
    );
    assert_eq!(s.lag_calendar(), LagCalendar::TwentyFourHour);
}

#[test]
fn setting_the_choice_keeps_the_cached_topological_order() {
    let (mut s, ..) = weekend_lag(LagCalendar::Successor);
    s.topological_order().unwrap();
    assert!(s.has_cached_order());
    for lag in CHOICES {
        s.set_lag_calendar(lag).unwrap();
        assert!(s.has_cached_order(), "{lag:?} dropped the cached order");
    }
    // A refused choice keeps it too.
    let _ = s.set_lag_calendar(LagCalendar::Fixed(CalendarId(9)));
    assert!(s.has_cached_order());
}

#[test]
fn changing_the_choice_changes_the_dates_of_the_same_schedule() {
    let (mut s, _, q, _) = weekend_lag(LagCalendar::Successor);
    let mut starts = Vec::new();
    for lag in CHOICES {
        s.set_lag_calendar(lag).unwrap();
        let sol = s.compute().unwrap();
        check_invariants(&s, &sol);
        starts.push(sol.get(q).es);
    }
    assert_eq!(
        starts,
        vec![at(7, 15, 0), at(8, 15, 0), at(5, 9, 0), at(8, 12, 0)]
    );
}

#[test]
fn a_cloned_schedule_keeps_its_choice() {
    let (s, _, q, start) = weekend_lag(LagCalendar::TwentyFourHour);
    let mut copy = s.clone();
    assert_eq!(copy.lag_calendar(), LagCalendar::TwentyFourHour);
    assert_eq!(copy.compute().unwrap().get(q).es, start);
}

/// A link that drives its successor under the choice is reported as driving.
/// `dependency_start` repeats the forward cases; read with the successor's
/// calendar it would put the start on Monday 15:00 and report nothing.
#[test]
fn the_driving_predecessor_is_found_under_every_choice() {
    for lag in CHOICES {
        let (mut s, p, q, start) = weekend_lag(lag);
        let sol = s.compute().unwrap();
        assert_eq!(sol.get(q).es, start, "{lag:?}");
        assert_eq!(s.driving_predecessors(&sol, q), vec![p], "{lag:?}");
        assert_eq!(s.driven_successors(&sol, p), vec![q], "{lag:?}");
    }
}

#[test]
fn the_longest_path_follows_the_link_under_every_choice() {
    for lag in CHOICES {
        let (mut s, p, q, _) = weekend_lag(lag);
        let sol = s.compute().unwrap();
        assert_eq!(s.longest_path(&sol), vec![p, q], "{lag:?}");
    }
}

/// Two predecessors on different calendars: A (Mon-Fri) ends Fri 17:00 after
/// five days, B (every day 08:00-12:00) ends Fri 12:00 after five mornings.
/// A links to C (Mon-Sat, one hour) with a 960 minute lag and B with 1260, so
/// the choice decides which link sets C's start, or whether both do.
#[test]
fn the_choice_picks_which_link_drives() {
    let build = |lag: LagCalendar| {
        let mut s = Schedule::new();
        let std = s.add_calendar(std_cal(&[]));
        let six = s.add_calendar(six_cal());
        let mornings = s.add_calendar(mornings_cal());
        let a = task(&mut s, std, 5 * 540, None);
        let b = task(&mut s, mornings, 5 * 240, None);
        let c = task(&mut s, six, 60, None);
        dep(&mut s, a, c, FS, 960);
        dep(&mut s, b, c, FS, 1260);
        s.set_lag_calendar(lag).unwrap();
        (s, a, b, c)
    };
    // (choice, C's start, the tasks that drive it)
    //   Successor (Mon-Sat): A's lag from Fri 17:00 is Sat 540 + Mon 420 and
    //     ends Mon 15:00; B's from Fri 12:00 is Fri 300 + Sat 540 + Mon 420
    //     and ends Mon 15:00 too. Both drive.
    //   Predecessor: A's (Mon-Fri) ends Tue 15:00. B's (240 a day) is Sat,
    //     Sun, Mon, Tue, Wed and 60 more: it ends Thu 09:00. B drives.
    //   24 h: A's ends Sat 09:00, B's 21 h from Fri 12:00 also Sat 09:00.
    //   Fixed (the mornings calendar): A's 960 from Fri 17:00 end Tue 12:00;
    //     B's is the Predecessor result, Thu 09:00. B drives.
    let cases: [(LagCalendar, i64, &[usize]); 4] = [
        (LagCalendar::Successor, at(7, 15, 0), &[0, 1]),
        (LagCalendar::Predecessor, at(10, 9, 0), &[1]),
        (LagCalendar::TwentyFourHour, at(5, 9, 0), &[0, 1]),
        (LagCalendar::Fixed(CalendarId(2)), at(10, 9, 0), &[1]),
    ];
    for (lag, start, drivers) in cases {
        let (mut s, a, b, c) = build(lag);
        let sol = s.compute().unwrap();
        check_invariants(&s, &sol);
        assert_eq!(sol.get(c).es, start, "{lag:?}");
        let mut got = s.driving_predecessors(&sol, c);
        got.sort();
        let want: Vec<TaskId> = drivers.iter().map(|&i| [a, b][i]).collect();
        assert_eq!(got, want, "{lag:?}");
    }
}
