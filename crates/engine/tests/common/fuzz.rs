//! Random small schedules for the brute-force comparison: up to 20 tasks,
//! random minute-resolution calendars (one always working Saturday and
//! Sunday), holidays, working exceptions (working weekend days, shortened and
//! lengthened weekdays), all four link types, negative and positive lags,
//! milestones and SNET constraints. Dependencies point from a lower index to
//! a higher one, as the oracle needs.

use engine::{
    Calendar, CalendarId, DayPeriod, DepId, Dependency, LagCalendar, LinkType, Rng, Schedule, Task,
    TaskId,
};

fn day_periods(rng: &mut Rng) -> Vec<DayPeriod> {
    let k = rng.range(1, 2) as usize;
    let mut cuts: Vec<u16> = Vec::new();
    while cuts.len() < 2 * k {
        let c = rng.range(0, 1440) as u16;
        if !cuts.contains(&c) {
            cuts.push(c);
        }
    }
    cuts.sort_unstable();
    cuts.chunks(2).map(|p| (p[0], p[1])).collect()
}

/// A random calendar. Working exceptions come from their own stream `ex`, so
/// `rng` draws the weeks, holidays and the rest of the schedule as it did
/// before working exceptions existed.
fn calendar(rng: &mut Rng, ex: &mut Rng, weekend: bool) -> Calendar {
    loop {
        let mut week: [Vec<DayPeriod>; 7] = Default::default();
        for (d, slot) in week.iter_mut().enumerate() {
            let p = if d < 5 { 0.85 } else { 0.35 };
            if (weekend && d >= 5) || rng.chance(p) {
                *slot = day_periods(rng);
            }
        }
        let mut holidays: Vec<i64> = (0..rng.range(0, 4)).map(|_| rng.range(0, 27)).collect();
        holidays.sort_unstable();
        holidays.dedup();
        let mut exceptions: Vec<(i64, Vec<DayPeriod>)> =
            holidays.into_iter().map(|d| (d, Vec::new())).collect();
        // Up to three working exceptions in the first four weeks, half of
        // them on a Saturday or Sunday; random periods, so a weekday one may
        // be shorter or longer than its pattern day.
        for _ in 0..ex.range(0, 3) {
            let day = if ex.chance(0.5) {
                7 * ex.range(0, 3) + ex.range(5, 6)
            } else {
                ex.range(0, 27)
            };
            let periods = day_periods(ex);
            if exceptions.iter().all(|&(d, _)| d != day) {
                exceptions.push((day, periods));
            }
        }
        let cal = Calendar::with_exceptions(week, &exceptions).expect("valid fuzz calendar");
        if cal.weekly_minutes() >= 600 {
            return cal;
        }
    }
}

/// A random schedule from `seed`: it also draws the lag calendar, one of
/// all four, with `Fixed` naming one of the schedule's calendars, and gives some dependencies a
/// lag calendar of their own (the same four). Both draws come after
/// every use of the main generator, so the tasks, calendars and dependencies of
/// a seed are the same as before the choice existed.
pub fn schedule(seed: u64) -> Schedule {
    let mut rng = Rng::new(seed ^ 0x005e_ed0f_f00d);
    let mut ex = Rng::new(seed ^ 0x0e8c_e971_0d5a);
    let mut s = Schedule::new();
    let ncal = rng.range(2, 4);
    let cals: Vec<CalendarId> = (0..ncal)
        .map(|i| {
            let c = calendar(&mut rng, &mut ex, i == 0);
            s.add_calendar(c)
        })
        .collect();
    let n = rng.range(2, 20) as u32;
    for _ in 0..n {
        let calendar = cals[rng.range(0, ncal - 1) as usize];
        let duration = if rng.chance(0.15) {
            0
        } else {
            rng.range(1, 300)
        };
        let snet = rng.chance(0.2).then(|| rng.range(0, 14 * 1440));
        s.add_task(Task {
            duration,
            calendar,
            snet,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("the generated task is valid");
    }
    for j in 1..n {
        let mut preds: Vec<u32> = Vec::new();
        for _ in 0..rng.range(0, 3) {
            let p = rng.range(0, j as i64 - 1) as u32;
            if preds.contains(&p) {
                continue;
            }
            preds.push(p);
            let kind =
                [LinkType::FS, LinkType::SS, LinkType::FF, LinkType::SF][rng.range(0, 3) as usize];
            let lag = if rng.chance(0.5) {
                0
            } else {
                rng.range(-300, 300)
            };
            s.add_dependency(Dependency {
                pred: TaskId(p),
                succ: TaskId(j),
                kind,
                lag,
            })
            .expect("the generated link is valid");
        }
    }
    let lag = match rng.range(0, 3) {
        0 => LagCalendar::Successor,
        1 => LagCalendar::Predecessor,
        2 => LagCalendar::TwentyFourHour,
        _ => LagCalendar::Fixed(cals[rng.range(0, ncal - 1) as usize]),
    };
    s.set_lag_calendar(lag).expect("a fuzz calendar exists");
    // A dependency may count its lag in a calendar of its own. A separate
    // generator, so the schedules above are the same as before this draw.
    let mut own = Rng::new(seed ^ 0x0b5e_55ed_1a65);
    for i in 0..s.dependencies().len() {
        if own.chance(0.3) {
            let choice = match own.range(0, 3) {
                0 => LagCalendar::Successor,
                1 => LagCalendar::Predecessor,
                2 => LagCalendar::TwentyFourHour,
                _ => LagCalendar::Fixed(cals[own.range(0, ncal - 1) as usize]),
            };
            s.set_dependency_lag_calendar(DepId(i as u32), Some(choice))
                .expect("a fuzz calendar exists");
        }
    }
    s
}

/// Progress for a schedule, drawn from `seed` on a stream of its own (so the schedule is the one
/// `schedule` made): a data date on most seeds, and a fraction `share` of the tasks each started
/// (with or without a remaining duration), or finished (usually with an actual start). Actual
/// dates fall in the first `span` minutes and may land outside working time, as typed dates do.
pub fn add_progress(s: &mut Schedule, seed: u64, share: f64, span: i64) {
    let mut rng = Rng::new(seed ^ 0x0ac7_0a15);
    s.set_data_date(rng.chance(0.7).then(|| rng.range(0, span)));
    for i in 0..s.tasks().len() {
        if !rng.chance(share) {
            continue;
        }
        let t = TaskId(i as u32);
        let start = rng.range(0, span);
        if rng.chance(0.4) {
            let done = start + rng.range(0, 5 * 1440);
            let from = (!rng.chance(0.1)).then_some(start);
            s.set_actual_start(t, from)
                .expect("the generated progress is valid");
            s.set_actual_finish(t, Some(done))
                .expect("the generated progress is valid");
        } else {
            s.set_actual_start(t, Some(start))
                .expect("the generated progress is valid");
            let left = rng.chance(0.6).then(|| rng.range(0, 300));
            s.set_remaining(t, left)
                .expect("the generated progress is valid");
        }
    }
}
