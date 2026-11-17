//! The slide rules (gs#481, story S3.1 of gs#427): how a task that an edit moved travels from its old
//! dates to its new ones.
//!
//! Pure functions on [`engine::Moved`] and [`engine::TaskDates`]. Nothing here keeps state or reads a
//! clock; the caller passes the time since the slide began. The rules:
//!
//! - an ease-out cubic ([`ease`]): fast first, settling at the end;
//! - a stagger ([`delay`]): the moved tasks start one after another in the order of their old start,
//!   the last one [`STAGGER_MS`] after the first, so a chain of changes reads as one wave;
//! - a blend ([`apply`]): early start and early finish move; late start, late finish and float stay
//!   the solution's, because a slide is a picture of where the bar was and not a different schedule.

use engine::{Moved, TaskDates};

/// How long one task takes to travel from its old dates to its new ones, in milliseconds.
pub const SLIDE_MS: f64 = 240.0;

/// The start delay of the last task behind the first when two or more tasks move, in milliseconds.
pub const STAGGER_MS: f64 = 120.0;

/// The ease-out cubic: `ease(0) = 0`, `ease(0.5) = 0.875`, `ease(1) = 1`. `p` outside `[0, 1]` is
/// clamped.
#[must_use]
pub fn ease(p: f64) -> f64 {
    let q = 1.0 - p.clamp(0.0, 1.0);
    1.0 - q * q * q
}

/// Milliseconds before the task at `rank` (0 is the earliest old start) of `count` starts to move.
/// Rank 0 starts at once, the last rank [`STAGGER_MS`] later, the ones between evenly spaced. One
/// task, or none, has no delay.
#[expect(
    clippy::cast_precision_loss,
    reason = "rank and count are counts of moved tasks, far under 2^53"
)]
#[must_use]
pub fn delay(rank: usize, count: usize) -> f64 {
    if count < 2 {
        return 0.0;
    }
    STAGGER_MS * rank.min(count - 1) as f64 / (count - 1) as f64
}

/// When a slide of `count` moved tasks ends, in milliseconds after it began: [`SLIDE_MS`] for one
/// task, [`SLIDE_MS`] + [`STAGGER_MS`] for two or more, 0 for none.
#[must_use]
pub fn total_ms(count: usize) -> f64 {
    if count == 0 {
        return 0.0;
    }
    delay(count - 1, count) + SLIDE_MS
}

/// Puts `entries` in the order [`apply`] expects: ascending old start, ties by task id. Call it once
/// when the slide starts, not on every frame.
pub fn sort_by_old_start(entries: &mut [Moved]) {
    entries.sort_unstable_by_key(|m| (m.was_es, m.task));
}

/// Writes the dates to draw `t_ms` milliseconds into the slide.
///
/// - `entries`: the moved tasks, sorted by [`sort_by_old_start`]; an entry's index is its rank for
///   [`delay`]. A task id past the end of `solution` or `scratch` is skipped.
/// - `solution`: the current dates, the target of every slide.
/// - `scratch`: a copy of `solution` made once when the slide began. [`apply`] overwrites the `es` and
///   `ef` of the tasks in `entries` and nothing else, so `ls`, `lf` and `float` stay the solution's
///   and a task not in `entries` keeps what `scratch` held.
///
/// Before its delay a task is at its old dates, after `delay + SLIDE_MS` at its new ones, and between
/// them on the [`ease`] curve. Early start and early finish blend separately, so a task whose
/// duration changed stretches while it slides.
pub fn apply(entries: &[Moved], solution: &[TaskDates], t_ms: f64, scratch: &mut [TaskDates]) {
    debug_assert!(
        entries.windows(2).all(|w| w[0].was_es <= w[1].was_es),
        "slide::apply: entries are not sorted by old start (call sort_by_old_start)"
    );
    let count = entries.len();
    for (rank, moved) in entries.iter().enumerate() {
        let i = moved.task.0 as usize;
        let (Some(new), Some(out)) = (solution.get(i), scratch.get_mut(i)) else {
            continue;
        };
        let e = ease((t_ms - delay(rank, count)) / SLIDE_MS);
        out.es = blend(moved.was_es, new.es, e);
        out.ef = blend(moved.was_ef, new.ef, e);
    }
}

/// `from` moved `e` of the way to `to`, rounded to a whole minute. `e = 0` gives `from`, `e = 1` gives
/// `to` exactly.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "to - from is a span of minutes, far under 2^53, so f64 holds it exactly; e is 0 to 1, so the product fits i64"
)]
fn blend(from: i64, to: i64, e: f64) -> i64 {
    from + ((to - from) as f64 * e).round() as i64
}
