//! The view a schedule opens with: fitted to the rows on its first screen.

use engine::Solution;
use timeline::{HIDDEN_ROW, Timeline};

/// Seven days of minutes: the view of a schedule with no rows.
const WEEK: i64 = 7 * 1440;

/// A timeline of `width` x `height` CSS pixels over rows `order`, fitted to the rows on its first screen: its view
/// runs from their earliest early start to their latest early finish.
#[must_use]
pub fn fitted_timeline(
    order: Vec<u32>,
    solution: &Solution,
    width: f64,
    height: f64,
    dpr: f64,
) -> Timeline {
    let mut timeline = Timeline::new(order, width, height, dpr);
    fit_first_screen(&mut timeline, solution);
    timeline
}

/// Set the view to run from the earliest start to the latest finish shown on the rows now on the first
/// screen: a summary row's rolled-up span, a leaf's early dates. A session calls it once per open and
/// again when the canvas width changes before the first wheel or fit (the ID column of the new file).
pub(crate) fn fit_first_screen(timeline: &mut Timeline, solution: &Solution) {
    let rows = timeline.viewport.visible_rows();
    let rows = &timeline.rows()[rows];
    let spans = rows.iter().filter(|&&t| t != HIDDEN_ROW).map(|&t| {
        timeline.summaries().get(t).unwrap_or_else(|| {
            let d = &solution.dates[t as usize];
            (d.es, d.ef)
        })
    });
    let start = spans.clone().map(|s| s.0).min().unwrap_or(0);
    // No rows (a blank schedule): its origin is the Monday of its first week, so the view shows that week.
    let finish = spans.map(|s| s.1).max().unwrap_or(WEEK).max(start);
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
    )]
    timeline.viewport.fit(start as f64, finish as f64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "an instant in a test schedule, far under 2^53 minutes, so f64 holds it exactly"
    )]
    fn a_fitted_timeline_covers_the_first_screen_rows_and_nothing_much_wider() {
        // The 50,000 task demo runs for years; its first screen shows rows that start together. The fit
        // is to those rows, so their bars fill the chart instead of sitting in its left corner.
        let (_, solution, order, _) = demo(50_000);
        let timeline = fitted_timeline(order, &solution, 1200.0, 800.0, 1.0);
        let shown = timeline.viewport.visible_rows();
        let rows: Vec<_> = timeline.order()[shown]
            .iter()
            .map(|&task| solution.dates[task as usize])
            .collect();
        let first = rows.iter().map(|d| d.es).min().unwrap() as f64;
        let last = rows.iter().map(|d| d.ef).max().unwrap() as f64;
        let (left, right) = timeline.viewport.visible_time();
        assert!(
            left <= first && last <= right,
            "the first screen's bars are in view"
        );
        assert!(
            (last - first) * 1.15 >= right - left,
            "the first screen's bars fill the chart: {} of {} minutes",
            last - first,
            right - left
        );
    }
}
