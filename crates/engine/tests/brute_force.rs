//! Property test: random small schedules (<= 20 tasks) with random
//! minute-resolution calendars, weekend-working predecessors, holidays and
//! working exceptions (working weekend days, shortened days),
//! solved by the engine and by the brute-force minute-by-minute oracle; every
//! date must match exactly.
//!
//! `GS_SEED=<n>` reruns one failing seed; `GS_SEEDS=<count>` widens the run.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

mod common;

use common::{fuzz, oracle, seeds};
use engine::generate::add_limits;
use engine::{DAY, Schedule};

#[test]
fn engine_matches_brute_force() {
    let seeds = seeds(400);
    for &seed in &seeds {
        let mut s = fuzz::schedule(seed);
        oracle::assert_matches(&mut s, &format!("seed {seed}"));
        // The same schedule once work is under way: actuals, remaining durations, a data date.
        fuzz::add_progress(&mut s, seed, 0.3, 12 * DAY);
        oracle::assert_matches(&mut s, &format!("seed {seed} with progress"));
        // Date limits on the plain schedule, and again over the progress.
        let mut s = fuzz::schedule(seed);
        add_limits(&mut s, seed, LIMIT_SHARE, LIMIT_SPAN);
        oracle::assert_matches(&mut s, &format!("seed {seed} with limits"));
        fuzz::add_progress(&mut s, seed, 0.3, 12 * DAY);
        oracle::assert_matches(&mut s, &format!("seed {seed} with limits and progress"));
    }
    eprintln!("brute force agreed on {} seeds", seeds.len());
}

/// A share of the tasks carry limits, drawn over the span the fuzz schedules cover.
const LIMIT_SHARE: f64 = 0.4;
const LIMIT_SPAN: i64 = 20 * DAY;

/// The limits reach their cases on the computed paths: a finish-no-earlier-than moves an early
/// start, a late limit cuts a late start, and one goes unmet (a negative float on a critical
/// task), also with a late start before the origin. Without this the comparison above could pass
/// on limits no date feels.
#[test]
fn fuzz_limits_cover_their_cases() {
    let (mut moves, mut cuts, mut negative, mut critical, mut before, mut pinned) =
        (0, 0, 0, 0, 0, 0);
    let mut late_placed = 0;
    for seed in 0..400 {
        let plain = fuzz::schedule(seed)
            .compute()
            .expect("fuzz schedule computes");
        let mut s = fuzz::schedule(seed);
        add_limits(&mut s, seed, LIMIT_SHARE, LIMIT_SPAN);
        let Ok(sol) = s.compute() else { continue };
        for (i, d) in sol.dates.iter().enumerate() {
            let (limit, was) = (s.tasks()[i].limit, plain.dates[i]);
            moves += (limit.fnet.is_some() && d.es != was.es) as i32;
            pinned += (limit.pin.is_some() && d.ef != was.ef) as i32;
            late_placed += (limit.alap && d.es > was.es) as i32;
            cuts += ((limit.snlt.is_some() || limit.fnlt.is_some()) && d.ls < was.ls) as i32;
            negative += (d.float < 0) as i32;
            critical += (d.float < 0 && d.is_critical()) as i32;
            before += (d.float < 0 && d.ls < 0) as i32;
        }
    }
    assert!(
        moves > 100
            && cuts > 100
            && negative > 100
            && critical == negative
            && before > 100
            && pinned > 100
            && late_placed > 100
    );
}

/// Review note (astra, #6): ten zero-duration FS tasks on alternating
/// Monday-only and Tuesday-only calendars reach day 29, past the oracle's
/// first horizon of 28 days (no work, no lag, no SNET, no holidays). Each
/// calendar transition waits up to a week, which the horizon did not count;
/// the oracle must still solve it.
#[test]
fn oracle_survives_calendar_transition_delays() {
    use engine::{Calendar, DayPeriod, LinkType, Schedule, Task};
    let only = |weekday: usize| {
        let mut week: [Vec<DayPeriod>; 7] = Default::default();
        week[weekday] = vec![(480, 1020)];
        Calendar::new(week, &[]).unwrap()
    };
    let mut s = Schedule::new();
    let cals = [s.add_calendar(only(0)), s.add_calendar(only(1))];
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            let calendar = cals[i % 2];
            s.add_task(Task {
                duration: 0,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .unwrap()
        })
        .collect();
    for w in tasks.windows(2) {
        s.add_dependency(engine::Dependency {
            pred: w[0],
            succ: w[1],
            kind: LinkType::FS,
            lag: 0,
        })
        .unwrap();
    }
    oracle::assert_matches(&mut s, "transition delays");
    let last = s.compute().unwrap().dates[9].es;
    assert_eq!(last, 29 * engine::DAY + 480, "Tuesday of the fifth week");
}

/// A finished task whose actual dates lie far beyond the oracle's first horizon (a few weeks for
/// two short tasks), beside an unfinished task that does not reach it: the oracle must retry with a longer horizon, not index past
/// the end of its calendar.
#[test]
fn oracle_survives_actual_dates_past_its_first_horizon() {
    let mut s = Schedule::new();
    let cal = s.add_calendar(common::std_cal(&[]));
    let done = common::task(&mut s, cal, 540, None);
    let open = common::task(&mut s, cal, 540, None);
    s.set_actual_start(done, Some(200 * DAY)).unwrap();
    s.set_actual_finish(done, Some(201 * DAY)).unwrap();
    oracle::assert_matches(&mut s, "actuals past the horizon");
    assert_eq!(s.compute().unwrap().get(open).es, 480);
}

/// The fuzz really produces the cases it claims to (a fuzz that never
/// generates them would pass vacuously).
#[test]
fn fuzz_covers_its_cases() {
    use engine::LinkType;
    let (mut kinds, mut neg, mut ms, mut snet, mut hol, mut weekend_preds) =
        ([0; 4], 0, 0, 0, 0, 0);
    for seed in 0..400 {
        let s = fuzz::schedule(seed);
        for d in s.dependencies() {
            kinds[match d.kind {
                LinkType::FS => 0,
                LinkType::SS => 1,
                LinkType::FF => 2,
                LinkType::SF => 3,
            }] += 1;
            neg += (d.lag < 0) as i32;
            let c = s.calendar_of(d.pred);
            weekend_preds += (!c.week()[5].is_empty() || !c.week()[6].is_empty()) as i32;
        }
        ms += s.tasks().iter().filter(|t| t.duration == 0).count();
        snet += s.tasks().iter().filter(|t| t.snet.is_some()).count();
        hol += s
            .calendars()
            .iter()
            .filter(|c| !c.holidays().is_empty())
            .count();
    }
    assert!(kinds.iter().all(|&k| k > 100), "link types {kinds:?}");
    assert!(neg > 100 && ms > 100 && snet > 100 && hol > 100 && weekend_preds > 100);
}

/// The index of a lag calendar choice, for the coverage counts.
fn choice_index(c: engine::LagCalendar) -> usize {
    match c {
        engine::LagCalendar::Successor => 0,
        engine::LagCalendar::Predecessor => 1,
        engine::LagCalendar::TwentyFourHour => 2,
        engine::LagCalendar::Fixed(_) => 3,
    }
}

/// The fuzz draws all four lag calendar choices, and each one is exercised
/// where it can matter: with a positive and a negative lag on a link between
/// tasks on different calendars, and on schedules whose dates differ from the
/// `Successor` result. Without this, a seed set where every choice gives the
/// same dates would pass the oracle comparison vacuously.
#[test]
fn fuzz_covers_every_lag_calendar() {
    let mut seeds_with = [0; 4];
    let mut positive = [0; 4];
    let mut negative = [0; 4];
    let mut differs = [0; 4];
    for seed in 0..400 {
        let mut s = fuzz::schedule(seed);
        let i = choice_index(s.lag_calendar());
        seeds_with[i] += 1;
        for d in s.dependencies() {
            let tasks = s.tasks();
            if tasks[d.pred.0 as usize].calendar != tasks[d.succ.0 as usize].calendar {
                positive[i] += (d.lag > 0) as i32;
                negative[i] += (d.lag < 0) as i32;
            }
        }
        let Ok(own) = s.compute() else { continue };
        let mut as_successor = s.clone();
        as_successor
            .set_lag_calendar(engine::LagCalendar::Successor)
            .unwrap();
        if as_successor.compute().is_ok_and(|sol| sol != own) {
            differs[i] += 1;
        }
    }
    for i in 0..4 {
        assert!(seeds_with[i] > 50, "choice {i}: seeds {seeds_with:?}");
        assert!(positive[i] > 10, "choice {i}: positive lags {positive:?}");
        assert!(negative[i] > 10, "choice {i}: negative lags {negative:?}");
    }
    for i in 1..4 {
        assert!(
            differs[i] > 10,
            "choice {i} never changes a date: {differs:?}"
        );
    }
    eprintln!("lag calendar coverage: seeds {seeds_with:?} differs {differs:?}");
}

/// Working exceptions sit on computed paths: some task in the fuzz works
/// inside a working weekend exception (the pattern has no minute that day),
/// and some inside a shortened day (fewer minutes than the pattern, but not
/// none). Without this, the comparison above could pass on exceptions that no
/// schedule ever crosses.
#[test]
fn fuzz_puts_working_exceptions_on_computed_paths() {
    use engine::DAY;
    let (mut weekend, mut shortened) = (0, 0);
    for seed in 0..400 {
        let mut s = fuzz::schedule(seed);
        let sol = s.compute().expect("fuzz schedule computes");
        for (i, task) in s.tasks().iter().enumerate() {
            let cal = s.calendar_of(engine::TaskId(i as u32));
            let d = &sol.dates[i];
            for (day, periods) in cal.exceptions() {
                let (a, b) = (d.es.max(day * DAY), d.ef.min((day + 1) * DAY));
                if periods.is_empty() || task.duration == 0 || a >= b {
                    continue;
                }
                if cal.working_between(a, b) == 0 {
                    continue;
                }
                let minutes = |p: &[engine::DayPeriod]| -> i64 {
                    p.iter().map(|&(s, e)| (e - s) as i64).sum()
                };
                let pattern = minutes(&cal.week()[(day % 7) as usize]);
                if pattern == 0 {
                    weekend += 1;
                } else if minutes(periods) < pattern {
                    shortened += 1;
                }
            }
        }
    }
    eprintln!(
        "tasks working in a working weekend exception {weekend}, in a shortened day {shortened}"
    );
    assert!(
        weekend >= 20 && shortened >= 20,
        "weekend {weekend}, shortened {shortened}"
    );
}

/// The progress the fuzz adds reaches its cases on the computed paths: started tasks with and
/// without a remaining duration, finished tasks, a data date, and a started task whose float
/// from the early finish differs from the float from the actual start. Without this, the
/// comparison could pass on actuals that change no date.
#[test]
fn fuzz_progress_covers_its_cases() {
    let (mut started, mut remaining, mut finished, mut dd, mut finish_only) = (0, 0, 0, 0, 0);
    let (mut float_differs, mut pinned_before_dd) = (0, 0);
    for seed in 0..400 {
        let mut s = fuzz::schedule(seed);
        fuzz::add_progress(&mut s, seed, 0.3, 12 * DAY);
        dd += s.data_date().is_some() as i32;
        for t in s.tasks() {
            started += (t.actual_start.is_some() && t.actual_finish.is_none()) as i32;
            remaining += (t.actual_start.is_some() && t.remaining.is_some()) as i32;
            finished += t.actual_finish.is_some() as i32;
            finish_only += (t.actual_finish.is_some() && t.actual_start.is_none()) as i32;
            let dd = s.data_date().unwrap_or(0);
            pinned_before_dd += t.actual_start.is_some_and(|a| a < dd) as i32;
        }
        let Ok(sol) = s.compute() else { continue };
        for (i, t) in s.tasks().iter().enumerate() {
            let d = sol.dates[i];
            let cal = s.calendar_of(engine::TaskId(i as u32));
            let started = t.actual_start.is_some() && t.actual_finish.is_none();
            // The early and late dates agree with the oracle, so these two counts are the rule's
            // two readings of the float, whatever float the engine reports.
            float_differs += (started
                && cal.working_between(d.ef, d.lf) != cal.working_between(d.es, d.ls))
                as i32;
        }
    }
    eprintln!(
        "started {started}, with remaining {remaining}, finished {finished} ({finish_only} with no start), data dates {dd}, started before the data date {pinned_before_dd}, float differs {float_differs}"
    );
    assert!(started > 300 && remaining > 100 && finished > 300 && finish_only > 10);
    assert!(dd > 200 && pinned_before_dd > 100 && float_differs > 50);
}
