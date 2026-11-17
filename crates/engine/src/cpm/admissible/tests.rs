//! `late_start` against a copy of its body from before the guess was probed first.

use super::{Admissible, late_start};
use crate::calendar::{Calendar, DAY, DayPeriod, Instant};
use crate::generate::{Rng, stress};
use crate::model::{DepId, ScheduleError, TaskId};
use std::cell::Cell;

/// `late_start` as it was when it probed `pred(e)` before `pred(g)`.
fn late_start_reference(
    a: &Admissible,
    task: TaskId,
    e: Instant,
    hi: Instant,
    guess: Instant,
    pred: impl Fn(Instant) -> Option<Instant>,
    blame: impl FnOnce() -> Option<DepId>,
) -> Result<(Instant, Instant), ScheduleError> {
    if pred(e).is_none() {
        return Err(ScheduleError::InvariantViolated {
            task,
            dep: blame(),
            es: e,
            ls: a.latest(0, hi, |x| pred(x).is_some()),
        });
    }
    let (mut lo, mut hi) = (e, hi);
    if let Some(g) = Some(a.prev(guess.min(hi))).filter(|&g| g >= lo) {
        if let Some(fin_g) = pred(g) {
            let after = a.next(g + 1);
            if after > hi || pred(after).is_none() {
                return Ok((g, fin_g));
            }
            lo = g;
        } else {
            hi = g - 1;
        }
    }
    let x = a
        .latest(lo, hi, |x| pred(x).is_some())
        .expect("lo satisfies pred");
    Ok((x, a.fin(x)))
}

fn pick<T: Copy>(rng: &mut Rng, items: &[T]) -> T {
    let i = rng.next_u64() % u64::try_from(items.len()).expect("a short list");
    items[usize::try_from(i).expect("a short list")]
}

fn minutes(rng: &mut Rng, lo: i64, hi: i64) -> u16 {
    u16::try_from(rng.range(lo, hi)).expect("a minute of the day")
}

/// A calendar with one period on most weekdays, and a few holidays.
fn calendar(rng: &mut Rng) -> Calendar {
    let mut week: [Vec<DayPeriod>; 7] = std::array::from_fn(|_| Vec::new());
    for (i, day) in week.iter_mut().enumerate() {
        if i == 0 || rng.chance(0.75) {
            let start = minutes(rng, 0, 1_000);
            day.push((start, start + minutes(rng, 1, 400)));
        }
    }
    let holidays: Vec<i64> = (0..rng.range(0, 5)).map(|_| rng.range(0, 120)).collect();
    Calendar::new(week, &holidays).expect("a valid calendar")
}

/// One random monotone predicate (the finish bound and up to three successor bounds, the lag
/// positive, zero or negative), run through both bodies; their results are equal, and the
/// predicate is called once less when the guess settles the early start.
#[test]
fn late_start_equals_its_reference_and_probes_less() {
    let mut rng = Rng::new(20_261_107);
    let (mut settled_cases, mut error_cases) = (0, 0);
    for _ in 0..20_000 {
        let (cal, lag_cals) = (calendar(&mut rng), [calendar(&mut rng), calendar(&mut rng)]);
        let duration = rng.range(0, 2_000) * i64::from(!rng.chance(0.2));
        let a = Admissible::new(&cal, duration);
        let e = a.next(rng.range(0, 40 * DAY));
        let pf = e + rng.range(-2_000, 40_000);
        // (bound is on the finish, lag, lag calendar, bound)
        let bounds: Vec<(bool, i64, usize, Instant)> = (0..rng.range(0, 3))
            .map(|_| {
                (
                    rng.chance(0.5),
                    rng.range(-600, 600),
                    pick(&mut rng, &[0, 1]),
                    e + rng.range(-2_000, 40_000),
                )
            })
            .collect();
        let pred_calls = Cell::new(0);
        let pred = |x: Instant| {
            pred_calls.set(pred_calls.get() + 1);
            let f = a.fin(x);
            (f <= pf
                && bounds.iter().all(|&(on_fin, lag, c, b)| {
                    lag_cals[c].add(if on_fin { f } else { x }, lag) <= b
                }))
            .then_some(f)
        };
        // As the backward pass bounds it, often with nothing to spare.
        let spare = rng.range(0, DAY);
        let slack = pick(&mut rng, &[0, 0, spare]);
        let hi = bounds
            .iter()
            .filter(|b| b.1 >= 0)
            .fold(pf, |hi, b| hi.min(b.3))
            + slack;
        let blame = || Some(DepId(7));
        let mut guess = rng.range(0, hi.max(1) + DAY);
        if rng.chance(0.7) {
            // A guess at, or next to, the answer: the path that skips the probe of `e`.
            if let Ok((ls, _)) = late_start_reference(&a, TaskId(1), e, hi, 0, pred, blame) {
                guess = ls + pick(&mut rng, &[0, 0, 1, -1, 60, -60]);
            }
        }
        pred_calls.set(0);
        let want = late_start_reference(&a, TaskId(1), e, hi, guess, pred, blame);
        let reference_calls = pred_calls.replace(0);
        let got = late_start(&a, TaskId(1), e, hi, guess, pred, blame);
        let calls = pred_calls.get();
        assert_eq!(got, want, "e {e} hi {hi} guess {guess} duration {duration}");

        let g = Some(a.prev(guess.min(hi))).filter(|&g| g >= e);
        let settled = usize::from(g.is_some_and(|g| pred(g).is_some()));
        // The extra call is the guess probed before the early start fails (the engine-bug path).
        let extra = usize::from(got.is_err() && g.is_some());
        assert_eq!(calls + settled, reference_calls + extra);
        (settled_cases, error_cases) = (settled_cases + settled, error_cases + extra);
    }
    assert!(settled_cases > 5_000 && error_cases > 500);
}

/// The FNV-1a digest of every `(es, ef, ls, lf, float)` of `compute()` on
/// `stress(20_261_020, 50_000)`, recorded on `main` before `late_start`
/// probed the guess first.
const STRESS_DIGEST: u64 = 0xfe27_7f33_5ed7_98b7;

#[test]
fn compute_on_stress_keeps_every_date() {
    let (mut s, _) = stress(20_261_020, 50_000);
    let sol = s.compute().expect("the stress schedule computes");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for d in &sol.dates {
        for v in [d.es, d.ef, d.ls, d.lf, d.float] {
            for b in v.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    assert_eq!(h, STRESS_DIGEST, "digest {h:#x}");
}
