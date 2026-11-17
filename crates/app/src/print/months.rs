//! The months a narrow printed timeline names (gantt#1755).

use super::PrintPlan;
use crate::Session;
use std::fmt::Write;
use timeline::axis::{self, Level};
use timeline::names::first_letter;

impl Session {
    /// The months' names, as `<text>` in the panel's coordinates, for the top half of an `axis` px tall band (gantt#1755). The axis names
    /// a month only where its ticks are 48 px apart and names the year alone below that: from 9 px a month is named, by its first letter, or its three from 28.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "an instant is minutes from the schedule origin, far under 2^53, so f64 holds it exactly; a float-to-int `as` saturates and never wraps"
    )]
    pub(super) fn month_names(&self, plan: &PrintPlan, axis: f64) -> String {
        let (scale, origin, mut out) = (plan.px_per_minute, self.origin.axis(), String::new());
        let names = &self.names;
        if axis::levels_for(scale).0 != Level::Year {
            return out;
        }
        let x = |t: i64| ((t as f64 - plan.t0) * scale).clamp(0.0, plan.panel_w);
        let to = plan.t0 + plan.panel_w / scale;
        for t in Level::Month.ticks(origin, plan.t0 as i64, to as i64) {
            let (lo, hi) = (x(t), x(Level::Month.next(origin, t)));
            if hi - lo < 9.0 {
                continue;
            }
            let name = Level::Month.short_label(origin, t, names);
            let name = if hi - lo < 28.0 {
                first_letter(&name)
            } else {
                &name
            };
            let at = f64::midpoint(lo, hi);
            let _ = write!(
                out,
                "<text x=\"{at}\" y=\"{}\" font-size=\"10\" text-anchor=\"middle\" fill=\"#1f2328\">{name}</text>",
                axis / 4.0 + 3.0
            );
        }
        out
    }
}
