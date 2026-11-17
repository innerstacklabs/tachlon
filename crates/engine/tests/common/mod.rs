//! Shared test support: instant helpers, small calendars, schedule builders,
//! the invariant checks every test runs, and the brute-force oracle.
#![allow(dead_code, reason = "each test binary uses part of the shared helpers")]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

pub mod cone_chain;
pub mod duration_chain;
pub mod equiv;
pub mod fuzz;
pub mod oracle;

use engine::{
    Admissible, Calendar, CalendarId, DAY, DayPeriod, Dependency, Instant, LagCalendar, LinkType,
    Schedule, Solution, Task, TaskId,
};

/// Instant of weekday-offset `day` (0 = the origin Monday), `h:m`.
pub fn at(day: i64, h: i64, m: i64) -> Instant {
    day * DAY + h * 60 + m
}

fn cal(days: &[usize], periods: &[DayPeriod], holidays: &[i64]) -> Calendar {
    let mut week: [Vec<DayPeriod>; 7] = Default::default();
    for &d in days {
        week[d] = periods.to_vec();
    }
    Calendar::new(week, holidays).expect("valid test calendar")
}

/// Mon-Fri 08:00-17:00.
pub fn std_cal(holidays: &[i64]) -> Calendar {
    cal(&[0, 1, 2, 3, 4], &[(480, 1020)], holidays)
}

/// Mon-Sat 08:00-17:00 (Saturday-working).
pub fn six_cal() -> Calendar {
    cal(&[0, 1, 2, 3, 4, 5], &[(480, 1020)], &[])
}

/// Every day 08:00-12:00.
pub fn mornings_cal() -> Calendar {
    cal(&[0, 1, 2, 3, 4, 5, 6], &[(480, 720)], &[])
}

/// 24/7.
pub fn cont_cal() -> Calendar {
    cal(&[0, 1, 2, 3, 4, 5, 6], &[(0, 1440)], &[])
}

pub fn task(s: &mut Schedule, cal: CalendarId, duration: i64, snet: Option<Instant>) -> TaskId {
    s.add_task(Task {
        duration,
        calendar: cal,
        snet,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    })
    .expect("valid task")
}

pub fn dep(s: &mut Schedule, pred: TaskId, succ: TaskId, kind: LinkType, lag: i64) {
    s.add_dependency(Dependency {
        pred,
        succ,
        kind,
        lag,
    })
    .expect("valid dependency");
}

/// Expected dates of one task, as written by hand in a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Want {
    pub es: Instant,
    pub ef: Instant,
    pub ls: Instant,
    pub lf: Instant,
    pub float: i64,
}

pub fn want(es: Instant, ef: Instant, ls: Instant, lf: Instant, float: i64) -> Want {
    Want {
        es,
        ef,
        ls,
        lf,
        float,
    }
}

pub fn assert_dates(sol: &Solution, t: TaskId, w: Want) {
    let d = sol.get(t);
    let got = Want {
        es: d.es,
        ef: d.ef,
        ls: d.ls,
        lf: d.lf,
        float: d.float,
    };
    assert_eq!(got, w, "dates of {t}");
}

/// The invariants every test checks on a computed schedule:
/// each forward and backward dependency inequality holds, es <= ls,
/// float >= 0, ef = fin(es), lf = fin(ls), es and ls are admissible starts,
/// nothing is before the origin, and the project finish has a zero-float
/// task ending at it.
pub fn check_invariants(s: &Schedule, sol: &Solution) {
    let tasks = s.tasks();
    let cal = |t: TaskId| s.calendar_of(t);
    let pf = sol.project_finish;
    for (i, task) in tasks.iter().enumerate() {
        let id = TaskId(i as u32);
        let d = sol.get(id);
        let a = Admissible::new(cal(id), task.duration);
        assert!(d.es >= 0, "{id}: es before origin");
        assert!(d.es <= d.ls, "{id}: es {} > ls {}", d.es, d.ls);
        assert!(d.float >= 0, "{id}: negative float");
        assert!(a.contains(d.es) && a.contains(d.ls), "{id}: inadmissible");
        assert_eq!(d.ef, a.fin(d.es), "{id}: ef != fin(es)");
        assert_eq!(d.lf, a.fin(d.ls), "{id}: lf != fin(ls)");
        assert!(d.lf <= pf, "{id}: lf past project finish");
        if let Some(c) = task.snet {
            assert!(d.es >= c, "{id}: SNET violated");
        }
    }
    for (k, dp) in s.dependencies().iter().enumerate() {
        let (p, q) = (sol.get(dp.pred), sol.get(dp.succ));
        // The calendar the lag is counted in: the dependency's own choice, else the schedule's.
        let all_day = cont_cal();
        let own = s.dependency_lag_calendar(engine::DepId(k as u32));
        let cs = match own.unwrap_or(s.lag_calendar()) {
            LagCalendar::Successor => cal(dp.succ),
            LagCalendar::Predecessor => cal(dp.pred),
            LagCalendar::TwentyFourHour => &all_day,
            LagCalendar::Fixed(id) => &s.calendars()[id.0 as usize],
        };
        let (fwd, bwd) = match dp.kind {
            LinkType::FS => (q.es >= cs.add(p.ef, dp.lag), cs.add(p.lf, dp.lag) <= q.ls),
            LinkType::SS => (q.es >= cs.add(p.es, dp.lag), cs.add(p.ls, dp.lag) <= q.ls),
            LinkType::FF => (q.ef >= cs.add(p.ef, dp.lag), cs.add(p.lf, dp.lag) <= q.lf),
            // A finish at a period end meets a bound at the next period's start.
            LinkType::SF => {
                let sc = cal(dp.succ);
                let meets = |x: Instant, f: Instant| sc.working_between(cs.add(x, dp.lag), f) >= 0;
                (meets(p.es, q.ef), meets(p.ls, q.lf))
            }
        };
        assert!(fwd, "dependency {k} ({dp:?}): forward inequality fails");
        assert!(bwd, "dependency {k} ({dp:?}): backward inequality fails");
    }
    if !tasks.is_empty() {
        assert!(
            sol.dates.iter().any(|d| d.ef == pf && d.float == 0),
            "no zero-float task ends at the project finish {pf}"
        );
        assert_eq!(
            pf,
            sol.dates
                .iter()
                .map(|d| d.ef)
                .max()
                .expect("the schedule has a task")
        );
    }
}

/// Assert `chain` is a zero-float chain linked by dependencies, starting at
/// the project start and ending at the project finish.
pub fn assert_critical_chain(s: &Schedule, sol: &Solution, chain: &[TaskId]) {
    let start = sol
        .dates
        .iter()
        .map(|d| d.es)
        .min()
        .expect("the schedule has a task");
    assert_eq!(
        sol.get(chain[0]).es,
        start,
        "chain does not start at the project start"
    );
    assert_eq!(
        sol.get(*chain.last().expect("the chain is not empty")).ef,
        sol.project_finish,
        "chain does not end at the project finish"
    );
    for &t in chain {
        assert_eq!(sol.get(t).float, 0, "{t} on the chain has float");
    }
    for w in chain.windows(2) {
        assert!(
            s.dependencies()
                .iter()
                .any(|d| d.pred == w[0] && d.succ == w[1]),
            "{} -> {} is not a dependency",
            w[0],
            w[1]
        );
    }
}

/// Seeds for the seeded property tests: `GS_SEED=<n>` runs one seed,
/// `GS_SEEDS=<count>` changes how many (default `default`).
pub fn seeds(default: u64) -> Vec<u64> {
    if let Ok(one) = std::env::var("GS_SEED") {
        return vec![one.parse().expect("GS_SEED is a number")];
    }
    let n = std::env::var("GS_SEEDS")
        .ok()
        .map_or(default, |v| v.parse().expect("GS_SEEDS is a number"));
    (0..n).collect()
}
