//! Brute-force minute-by-minute solver, used only in tests.
//!
//! It shares the plan's definitions with the engine but none of its code:
//! each calendar is expanded into one boolean per minute over a horizon
//! (straight from the weekly periods and the exception days, whose periods
//! replace the week's on their date), `add` reads a
//! per-minute running count, and every early start and late start is found
//! by testing each minute in turn: upward from the origin for es, downward
//! from the project finish for ls. It needs every dependency to point from a
//! lower task index to a higher one, which the fuzz schedules guarantee.
//!
//! Progress (decision 0008, written out here from the decision, not from `Pass`): a finished task
//! sits at its actuals with float 0 and binds nobody; a started task keeps its actual start, its
//! remaining work (else the whole duration) is placed by the same minute search, not before the
//! data date, and its float is counted from its early finish to its late finish; an unstarted
//! task may not start before the data date. Retained logic: the work of a started task still waits
//! for every predecessor. A finished successor binds no predecessor, and a start-to-start or
//! start-to-finish successor of a started task binds the fixed actual start, which is already met.
//!
//! The horizon is a first guess: a search that reaches its end marks the run
//! exhausted, and [`solve`] repeats it with the horizon doubled, so a short
//! guess costs time, never a rejected schedule.

use std::cell::Cell;

use engine::{
    Calendar, DAY, DepId, Instant, LagCalendar, Limit, LinkType, Pin, Schedule, TaskId, WEEK,
};

pub struct MinuteCal {
    /// The tables start this many minutes before the origin, where the weekly pattern runs on
    /// (a late date can fall there): instant `t` is entry `t + before`.
    before: i64,
    working: Vec<bool>,
    /// `cum[t + before]` = working minutes in `[-before, t)`.
    cum: Vec<i64>,
    /// Set when an `add` needed minutes past either end of the tables; its result is then
    /// meaningless and the whole run is discarded.
    exhausted: Cell<bool>,
}

impl MinuteCal {
    pub fn new(cal: &Calendar, horizon: i64) -> MinuteCal {
        let mut working = Vec::with_capacity(2 * horizon as usize);
        for t in -horizon..horizon {
            let day = t.div_euclid(DAY);
            let minute = t.rem_euclid(DAY) as u16;
            let periods = match cal.exceptions().iter().find(|(d, _)| *d == day) {
                Some((_, periods)) => periods,
                None => &cal.week()[day.rem_euclid(7) as usize],
            };
            working.push(periods.iter().any(|&(s, e)| s <= minute && minute < e));
        }
        let mut cum = vec![0i64; working.len() + 1];
        for t in 0..working.len() {
            cum[t + 1] = cum[t] + working[t] as i64;
        }
        MinuteCal {
            before: horizon,
            working,
            cum,
            exhausted: Cell::new(false),
        }
    }

    /// A calendar that works every minute of the horizon.
    pub fn all_minutes(horizon: i64) -> MinuteCal {
        MinuteCal {
            before: horizon,
            working: vec![true; 2 * horizon as usize],
            cum: (0..=2 * horizon).collect(),
            exhausted: Cell::new(false),
        }
    }

    fn at(&self, t: Instant) -> usize {
        (t + self.before) as usize
    }

    pub fn working(&self, t: Instant) -> bool {
        self.working[self.at(t)]
    }

    /// Admissible start for a task of this duration. The origin is no period end.
    pub fn admissible(&self, t: Instant, duration: i64) -> bool {
        self.working(t) || (duration == 0 && t != 0 && t > -self.before && self.working(t - 1))
    }

    /// `w > 0`: smallest `u` with `w` working minutes in `[t, u)`;
    /// `w < 0`: largest `u` with `-w` working minutes in `[u, t)`.
    pub fn add(&self, t: Instant, w: i64) -> Instant {
        let c = self.cum[self.at(t)];
        if w == 0 {
            t
        } else if w > 0 {
            let target = c + w;
            if *self.cum.last().expect("the table has an entry") < target {
                self.exhausted.set(true);
                return self.working.len() as Instant - self.before;
            }
            self.cum.partition_point(|&x| x < target) as Instant - self.before
        } else if c + w < 0 {
            self.exhausted.set(true);
            -self.before
        } else {
            let target = c + w;
            self.cum.partition_point(|&x| x <= target) as Instant - 1 - self.before
        }
    }

    pub fn between(&self, a: Instant, b: Instant) -> i64 {
        self.cum[self.at(b)] - self.cum[self.at(a)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dates {
    pub es: Instant,
    pub ef: Instant,
    pub ls: Instant,
    pub lf: Instant,
    pub float: i64,
}

/// The start and the finish a task is pinned to.
fn pin_dates(l: Limit) -> (Option<Instant>, Option<Instant>) {
    match l.pin {
        Some(Pin::Start(c)) => (Some(c), None),
        Some(Pin::Finish(c)) => (None, Some(c)),
        None => (None, None),
    }
}

/// The first horizon tried: enough for the work, lags, SNETs and exception days,
/// but not for calendar transitions (each can wait up to a week), so
/// [`solve`] grows it when a search reaches its end.
fn horizon(s: &Schedule) -> i64 {
    let tasks = s.tasks();
    let max_snet = tasks
        .iter()
        .flat_map(|t| {
            let (start, finish) = pin_dates(t.limit);
            [t.snet, t.limit.fnet, start, finish]
        })
        .flatten()
        .max()
        .unwrap_or(0);
    let work: i64 = tasks.iter().map(|t| t.duration).sum::<i64>()
        + s.dependencies().iter().map(|d| d.lag.max(0)).sum::<i64>();
    let min_weekly = s
        .calendars()
        .iter()
        .map(engine::Calendar::weekly_minutes)
        .min()
        .expect("the schedule has a calendar");
    let exceptions = s
        .calendars()
        .iter()
        .map(|c| c.exceptions().len() as i64)
        .max()
        .expect("the schedule has a calendar");
    max_snet + (2 * work / min_weekly + 4) * WEEK + exceptions * DAY
}

/// Solve by brute force; returns per-task dates and the project finish.
/// Doubles the horizon until no search reaches its end.
pub fn solve(s: &Schedule) -> (Vec<Dates>, Instant) {
    let mut h = horizon(s);
    for _ in 0..8 {
        if let Some(solved) = solve_within(s, h) {
            return solved;
        }
        h *= 2;
    }
    panic!("oracle: no solution within a horizon of {h} minutes");
}

/// Solve within horizon `h`; `None` when a search reached its end.
#[expect(
    clippy::too_many_lines,
    reason = "the oracle is one pass over the schedule, kept whole to read against the rules it checks"
)]
#[expect(
    clippy::many_single_char_names,
    reason = "the names are the task index, the instant, the dependency and its successor, used through the whole oracle"
)]
fn solve_within(s: &Schedule, h: i64) -> Option<(Vec<Dates>, Instant)> {
    let cals: Vec<MinuteCal> = s.calendars().iter().map(|c| MinuteCal::new(c, h)).collect();
    let all_day = MinuteCal::all_minutes(h);
    let exhausted = || all_day.exhausted.get() || cals.iter().any(|c| c.exhausted.get());
    let tasks = s.tasks();
    let n = tasks.len();
    let cal = |i: usize| &cals[tasks[i].calendar.0 as usize];
    let deps = s.dependencies();
    for d in deps {
        assert!(
            d.pred.0 < d.succ.0,
            "oracle needs index-ordered dependencies"
        );
    }
    // The working minutes still to schedule: what remains once a task has started, else all of it.
    let work = |i: usize| match (tasks[i].actual_start, tasks[i].remaining) {
        (Some(_), Some(r)) => r,
        _ => tasks[i].duration,
    };
    let dd = s.data_date().unwrap_or(0);
    let finished = |i: usize| tasks[i].actual_finish.is_some();
    let started = |i: usize| tasks[i].actual_start.is_some();
    let fin = |i: usize, x: Instant| cal(i).add(x, work(i));
    // The calendar a dependency's lag is counted in: the rule, read straight
    // off the dependency's own choice, else the schedule's (the engine's `Pass::lag_cal` is not used).
    let lag_cal = |di: usize, d: &engine::Dependency| match s
        .dependency_lag_calendar(DepId(di as u32))
        .unwrap_or(s.lag_calendar())
    {
        LagCalendar::Successor => cal(d.succ.0 as usize),
        LagCalendar::Predecessor => cal(d.pred.0 as usize),
        LagCalendar::TwentyFourHour => &all_day,
        LagCalendar::Fixed(id) => &cals[id.0 as usize],
    };
    // The dates a task is pinned to, snapped to working time by scanning the minutes: a start
    // to the first admissible start at or after it (none once started), a finish to the last
    // end of a working minute at or before it. Both passes use the snapped dates.
    let pins = |i: usize| {
        let (start, finish) = pin_dates(tasks[i].limit);
        let start = start
            .filter(|_| !started(i))
            .map(|p| (p..h).find(|&x| cal(i).admissible(x, work(i))).unwrap_or(h));
        let finish = finish.map(|p| {
            (1..=p.min(h))
                .rev()
                .find(|&e| cal(i).working(e - 1))
                .unwrap_or(p)
        });
        (start, finish)
    };
    let mut es = vec![0; n];
    let mut ef = vec![0; n];
    for i in 0..n {
        if let Some(done) = tasks[i].actual_finish {
            es[i] = tasks[i].actual_start.unwrap_or(done).max(0);
            ef[i] = done.max(es[i]);
            continue;
        }
        let c = cal(i);
        // Where the (remaining) work may begin: not before the data date, nor, for a started
        // task, before it did; the SNET binds only an unstarted task.
        let floor = match tasks[i].actual_start {
            Some(a) => a.max(0).max(dd),
            None => tasks[i].snet.unwrap_or(0).max(dd),
        };
        // A pin holds its date: the start pin bounds the start (not once started), the finish
        // pin the finish, and no dependency binds the task.
        let (start_pin, finish_pin) = pins(i);
        let held = start_pin.or(finish_pin).is_some();
        let ok = |x: Instant| {
            c.admissible(x, work(i))
                && x >= floor
                && start_pin.is_none_or(|p| x >= p)
                && [tasks[i].limit.fnet, finish_pin]
                    .iter()
                    .flatten()
                    .all(|&f| fin(i, x) >= f)
                && deps
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| !held && d.succ == TaskId(i as u32))
                    .all(|(di, d)| {
                        let p = d.pred.0 as usize;
                        let cl = lag_cal(di, d);
                        match d.kind {
                            LinkType::FS => x >= cl.add(ef[p], d.lag),
                            LinkType::SS => x >= cl.add(es[p], d.lag),
                            LinkType::FF => fin(i, x) >= cl.add(ef[p], d.lag),
                            // No working minute of S from the bound to its finish is short.
                            LinkType::SF => c.between(cl.add(es[p], d.lag), fin(i, x)) >= 0,
                        }
                    })
        };
        let x = (0..h).find(|&x| ok(x))?;
        es[i] = tasks[i].actual_start.map_or(x, |a| a.max(0));
        ef[i] = fin(i, x);
    }
    // Actual dates are taken as typed, whatever the horizon: one past its end is a first guess
    // too short, like a search that reaches the end.
    if exhausted() || es.iter().chain(&ef).any(|&x| x >= h) {
        return None;
    }
    let pf = ef.iter().copied().max().unwrap_or(0);
    // The backward pass runs from the earlier of the must-finish-by date and the project finish.
    let anchor = s.finish_by().map_or(pf, |d| d.min(pf));
    // The latest start of task `i` that holds against the dates `(ls, lf)` of its successors, found
    // by testing each minute down from `top` (the anchor, or the project finish for ALAP).
    let latest = |i: usize, ls: &[Instant], lf: &[Instant], top: Instant| {
        let c = cal(i);
        let (start_pin, finish_pin) = pins(i);
        let ok = |x: Instant| {
            c.admissible(x, work(i))
                && fin(i, x) <= top
                && [tasks[i].limit.fnlt, finish_pin]
                    .iter()
                    .flatten()
                    .all(|&f| fin(i, x) <= f)
                && (started(i)
                    || [tasks[i].limit.snlt, start_pin]
                        .iter()
                        .flatten()
                        .all(|&l| x <= l))
                && deps
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.pred == TaskId(i as u32))
                    .filter(|(_, d)| {
                        !(finished(d.succ.0 as usize)
                            || (started(i) && matches!(d.kind, LinkType::SS | LinkType::SF)))
                    })
                    .all(|(di, d)| {
                        let q = d.succ.0 as usize;
                        let cl = lag_cal(di, d);
                        match d.kind {
                            LinkType::FS => cl.add(fin(i, x), d.lag) <= ls[q],
                            LinkType::SS => cl.add(x, d.lag) <= ls[q],
                            LinkType::FF => cl.add(fin(i, x), d.lag) <= lf[q],
                            LinkType::SF => cal(q).between(lf[q], cl.add(x, d.lag)) <= 0,
                        }
                    })
        };
        (-h..=top).rev().find(|&x| ok(x))
    };
    // As late as possible (gantt#1875): an unstarted task so marked moves, last task first, to
    // the latest start that holds with its successors' early dates standing as their late
    // dates, so one that cannot be placed later (or only earlier) stays.
    for i in (0..n).rev() {
        if tasks[i].limit.alap
            && !started(i)
            && !finished(i)
            && let Some(x) = latest(i, &es, &ef, pf).filter(|&x| x > es[i])
        {
            (es[i], ef[i]) = (x, fin(i, x));
        }
    }
    let mut ls = vec![0; n];
    let mut lf = vec![0; n];
    for i in (0..n).rev() {
        if finished(i) {
            (ls[i], lf[i]) = (es[i], ef[i]);
            continue;
        }
        // No start in the tables meets every bound: a late limit put the late start further
        // before the origin than they reach, or a search reached their end.
        let Some(x) = latest(i, &ls, &lf, anchor) else {
            assert!(has_limits(s) || exhausted(), "oracle: no late start");
            return None;
        };
        ls[i] = x;
        lf[i] = fin(i, ls[i]);
    }
    if exhausted() {
        return None;
    }
    let dates = (0..n)
        .map(|i| Dates {
            es: es[i],
            ef: ef[i],
            ls: ls[i],
            lf: lf[i],
            float: if finished(i) {
                0
            } else if started(i) {
                cal(i).between(ef[i], lf[i])
            } else {
                cal(i).between(es[i], ls[i])
            },
        })
        .collect();
    Some((dates, pf))
}

/// Whether any task carries a date limit.
pub fn has_limits(s: &Schedule) -> bool {
    s.finish_by().is_some() || s.tasks().iter().any(|t| t.limit != Limit::default())
}

/// Whether any task has an actual date or a remaining duration.
pub fn has_progress(s: &Schedule) -> bool {
    s.tasks()
        .iter()
        .any(|t| t.actual_start.is_some() || t.actual_finish.is_some() || t.remaining.is_some())
}

/// Compare the engine with the oracle on one schedule; panics with `label`.
pub fn assert_matches(s: &mut Schedule, label: &str) {
    let sol = s
        .compute()
        .unwrap_or_else(|e| panic!("{label}: engine failed: {e}"));
    // `check_invariants` assumes `ef == fin(es)` with the planned duration, which a pinned bar
    // does not satisfy; the oracle comparison below covers progress.
    if !has_progress(s) && !has_limits(s) {
        super::check_invariants(s, &sol);
    }
    let (want, pf) = solve(s);
    assert_eq!(sol.project_finish, pf, "{label}: project finish");
    for (i, w) in want.iter().enumerate() {
        let d = sol.dates[i];
        let got = Dates {
            es: d.es,
            ef: d.ef,
            ls: d.ls,
            lf: d.lf,
            float: d.float,
        };
        assert_eq!(got, *w, "{label}: task {i}");
        let finished = s.tasks()[i].actual_finish.is_some();
        assert_eq!(
            d.is_critical(),
            w.float <= 0 && !finished,
            "{label}: task {i} critical"
        );
    }
}
