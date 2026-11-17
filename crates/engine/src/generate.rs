//! Deterministic schedule generator (plan, "Engine (M1)").
//!
//! Shape: parallel work streams (think buildings or areas), each a sequence of
//! WBS-like phases. Inside a phase a task depends on one or two recent tasks
//! of the same phase; each phase starts from a task of the stream's previous
//! phase; a few long links join arbitrary earlier tasks. About 1.5
//! dependencies per task, four calendars, some lags, some SNET constraints,
//! a few milestones. Task ids are a seeded permutation of generation order,
//! so the topological sort has real work to do.
//!
//! [`generate`] takes the `Shape::Packages` variant: a stream is
//! several work packages that overlap, each phase starting from the phase
//! `PHASE_LAG` before it, so a 50 000-task schedule spans a few years instead
//! of decades. [`stress`] keeps `Shape::Chain`, the shape above, so the
//! measurement series does not move (gs#320). [`generate_demo`] is `Packages` on whole hours.
//!
//! [`stress`] adds dense cross-phase dependencies and one head task that
//! every stream depends on, the drag target in M3.
//!
//! Every value comes from [`Rng`] (`SplitMix64`), so a seed gives the same
//! schedule on every platform and toolchain.

mod calendars;
mod limits;

use calendars::add_calendars;
pub use limits::add_limits;

use crate::calendar::DAY;
use crate::model::{CalendarId, Dependency, LinkType, Schedule, Task, TaskId};

/// `SplitMix64`: a small, fast, deterministic generator.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator from a seed.
    #[must_use]
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `lo..=hi`.
    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        reason = "lo <= hi is a debug_assert, not a check, so hi - lo is non-negative; the \
                  remainder is below span, far below i64::MAX for the generator's small ranges"
    )]
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi - lo) as u64 + 1;
        lo + (self.next_u64() % span) as i64
    }

    /// True with probability `p`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the top 53 bits of a u64 and 2^53 are both exact in f64"
    )]
    pub fn chance(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }
}

/// A phase starts from the phase this many before it in its stream (the
/// stream's first phase for the first few), so a stream is several work
/// packages running side by side rather than one chain; the schedule then
/// spans a few years instead of decades.
const PHASE_LAG: usize = 3;

/// How a stream's phases and links are drawn. Only the demo passes `Packages` or `Hourly`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// The shape of the stress fixture: a stream is one chain of phases, each
    /// starting from the one before, with short in-phase links. Its draw
    /// sequence and output are fixed (the stress timings are a series).
    Chain,
    /// The demo: a stream is several overlapping work packages.
    Packages,
    /// [`Shape::Packages`] on whole hours: see [`generate_demo`].
    Hourly,
}

struct Draft {
    tasks: Vec<Task>,
    deps: Vec<(usize, usize, LinkType, i64)>,
    /// First task of each phase, per stream.
    stream_starts: Vec<usize>,
}

#[expect(clippy::too_many_lines, reason = "tasks and links in one seeded pass")]
fn draft(rng: &mut Rng, n: usize, cals: [CalendarId; 4], dense: bool, shape: Shape) -> Draft {
    let packages = shape != Shape::Chain;
    let mut tasks = Vec::with_capacity(n);
    let mut deps = Vec::with_capacity(n * 2);
    let mut stream_starts = Vec::new();
    let streams = (n / 2500).max(1);
    let per_stream = n.div_ceil(streams);
    let mut phase_starts: Vec<usize> = Vec::new();
    let mut i = 0;
    for _ in 0..streams {
        let stream_end = (i + per_stream).min(n);
        // The stream's phases so far, as (start, end) pairs.
        let mut phases: Vec<(usize, usize)> = Vec::new();
        let stream_first = i;
        stream_starts.push(i);
        while i < stream_end {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the draw is in 20..=120"
            )]
            let len = rng.range(20, 120) as usize;
            let (start, end) = (i, (i + len).min(stream_end));
            phase_starts.push(start);
            for t in start..end {
                let pick = rng.range(0, 99);
                let calendar = cals[if pick < 70 {
                    0
                } else if pick < 85 {
                    1
                } else if pick < 93 {
                    3
                } else {
                    2
                }];
                let duration = if rng.chance(0.03) {
                    0
                } else {
                    rng.range(1, 40) * 120
                };
                let snet_days = if packages { 90 } else { 3 * 365 };
                let snet = rng.chance(0.02).then(|| rng.range(0, snet_days) * DAY);
                tasks.push(Task {
                    duration,
                    calendar,
                    snet,
                    limit: crate::model::Limit::default(),
                    deleted: false,
                    actual_start: None,
                    actual_finish: None,
                    remaining: None,
                });
                let mut chosen: Vec<usize> = Vec::new();
                let mut link = |rng: &mut Rng, p: usize, chosen: &mut Vec<usize>| {
                    if chosen.contains(&p) {
                        return;
                    }
                    chosen.push(p);
                    let k = rng.range(0, 99);
                    let kind = if k < 80 {
                        LinkType::FS
                    } else if k < 90 {
                        LinkType::SS
                    } else if shape == Shape::Hourly {
                        // A finish-side link can bound a finish off its closing hour: "08:01".
                        LinkType::FS
                    } else if k < 97 {
                        LinkType::FF
                    } else {
                        LinkType::SF
                    };
                    let lag = if rng.chance(0.2) {
                        rng.range(-8, 16) * 60
                    } else {
                        0
                    };
                    deps.push((p, t, kind, lag));
                };
                if t == start {
                    let from = if packages {
                        phases
                            .len()
                            .checked_sub(PHASE_LAG)
                            .and_then(|k| phases.get(k))
                            .or(phases.first())
                    } else {
                        phases.last()
                    };
                    if let Some(&(ps, pe)) = from {
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            clippy::cast_possible_wrap,
                            reason = "ps < pe are task indices below n, and the draw is in \
                                      midpoint(ps, pe)..=pe - 1, non-negative and below n"
                        )]
                        let p = rng.range(usize::midpoint(ps, pe) as i64, pe as i64 - 1) as usize;
                        link(rng, p, &mut chosen);
                    }
                } else {
                    let lo = if packages {
                        start
                    } else {
                        start.max(t.saturating_sub(8))
                    };
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        clippy::cast_possible_wrap,
                        reason = "lo < t are task indices below n, and the draw is in lo..=t - 1, \
                                  non-negative and below n"
                    )]
                    let p = rng.range(lo as i64, t as i64 - 1) as usize;
                    link(rng, p, &mut chosen);
                    if rng.chance(0.45) {
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            clippy::cast_possible_wrap,
                            reason = "lo < t are task indices below n, and the draw is in lo..=t - \
                                      1, non-negative and below n"
                        )]
                        let p = rng.range(lo as i64, t as i64 - 1) as usize;
                        link(rng, p, &mut chosen);
                    }
                }
                // Chain: a long link to any earlier task. Packages: to an earlier
                // task of the same stream.
                let floor = if packages { stream_first } else { 0 };
                if t > floor && rng.chance(0.03) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        clippy::cast_possible_wrap,
                        reason = "t > floor, both task indices below n, and the draw is in \
                                  floor..=t - 1, non-negative and below n"
                    )]
                    let p = rng.range(floor as i64, t as i64 - 1) as usize;
                    link(rng, p, &mut chosen);
                }
                if dense && phase_starts.len() > 1 {
                    for _ in 0..2 {
                        // a task from an earlier phase, anywhere in the schedule
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            clippy::cast_possible_wrap,
                            reason = "phase_starts.len() > 1 here, so the draw is in 0..=len - 2, \
                                      non-negative and below the length"
                        )]
                        let ph = rng.range(0, phase_starts.len() as i64 - 2) as usize;
                        let (first, next) = (phase_starts[ph], phase_starts[ph + 1]);
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            clippy::cast_possible_wrap,
                            reason = "first < next are consecutive phase starts below n, and the draw \
                                      is in first..=next - 1, non-negative and below n"
                        )]
                        let p = rng.range(first as i64, next as i64 - 1) as usize;
                        link(rng, p, &mut chosen);
                    }
                }
            }
            phases.push((start, end));
            i = end;
        }
    }
    Draft {
        tasks,
        deps,
        stream_starts,
    }
}

fn assemble(rng: &mut Rng, d: &Draft, cals_first: Schedule) -> (Schedule, Vec<TaskId>) {
    let n = d.tasks.len();
    // seeded Fisher-Yates permutation: generation index -> task id
    let mut perm: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_possible_wrap,
            reason = "i is in 1..n, an index below n, and the draw is in 0..=i, non-negative and \
                      below n"
        )]
        perm.swap(i, rng.range(0, i as i64) as usize);
    }
    let mut inv = vec![0usize; n];
    for (g, &id) in perm.iter().enumerate() {
        inv[id] = g;
    }
    let mut s = cals_first;
    for &g in &inv {
        s.add_task(d.tasks[g].clone())
            .expect("generated tasks are valid");
    }
    for &(p, t, kind, lag) in &d.deps {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "perm holds task ids of the n generated tasks, and the generator's n is far \
                      below 2^32 (the app asks for 50000)"
        )]
        let dep = Dependency {
            pred: TaskId(perm[p] as u32),
            succ: TaskId(perm[t] as u32),
            kind,
            lag,
        };
        s.add_dependency(dep)
            .expect("generated dependencies are valid");
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "perm holds task ids of the n generated tasks, and the generator's n is far below \
                  2^32 (the app asks for 50000)"
    )]
    let ids = (0..n).map(|g| TaskId(perm[g] as u32)).collect();
    (s, ids)
}

/// A generated schedule of `n` tasks.
#[must_use]
pub fn generate(seed: u64, n: usize) -> Schedule {
    let mut rng = Rng::new(seed);
    let mut s = Schedule::new();
    let cals = add_calendars(&mut s, &mut rng, None, false);
    let d = draft(&mut rng, n, cals, false, Shape::Packages);
    assemble(&mut rng, &d, s).0
}

/// The schedule the app's demo opens (`?n=50000`): [`generate`]'s shape with every start and
/// finish on a whole working hour (gs#972). Calendar 3 closes at 16:00 rather than 16:30, and
/// links are FS and SS only (the draws are the same, so the tasks are `generate`'s).
/// [`generate`] keeps all four link types for the engine's tests and benches.
#[must_use]
pub fn generate_demo(seed: u64, n: usize) -> Schedule {
    let mut rng = Rng::new(seed);
    let mut s = Schedule::new();
    let cals = add_calendars(&mut s, &mut rng, None, true);
    let d = draft(&mut rng, n, cals, false, Shape::Hourly);
    assemble(&mut rng, &d, s).0
}

/// The stress fixture: `n` tasks with dense cross-phase dependencies, plus a
/// head task (returned) at the head of a chain that reaches most of the
/// schedule. `n` counts the head.
#[must_use]
pub fn stress(seed: u64, n: usize) -> (Schedule, TaskId) {
    stress_from(seed, n, None)
}

/// The stress fixture of [`stress`] with working exceptions on calendars 0
/// and 3: about ten a year, by turns a working Saturday morning and a
/// Wednesday cut to the morning. The tasks, links and holidays are those of
/// `stress(seed, n)`; only the calendars differ.
#[must_use]
pub fn stress_with_exceptions(seed: u64, n: usize) -> (Schedule, TaskId) {
    stress_from(seed, n, Some(&mut Rng::new(seed ^ 0x0e8c_e971_0d5a)))
}

fn stress_from(seed: u64, n: usize, working: Option<&mut Rng>) -> (Schedule, TaskId) {
    assert!(n >= 2, "stress fixture needs at least two tasks");
    let mut rng = Rng::new(seed);
    let mut s = Schedule::new();
    let cals = add_calendars(&mut s, &mut rng, working, false);
    let mut d = draft(&mut rng, n - 1, cals, true, Shape::Chain);
    // Head: generation index n - 1, linked FS into the first task of every
    // stream, so every task of every stream depends on it.
    let head = d.tasks.len();
    d.tasks.push(Task {
        duration: 480,
        calendar: cals[0],
        snet: None,
        limit: crate::model::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    });
    for &st in &d.stream_starts.clone() {
        d.deps.push((head, st, LinkType::FS, 0));
    }
    let (s, ids) = assemble(&mut rng, &d, s);
    (s, ids[head])
}
