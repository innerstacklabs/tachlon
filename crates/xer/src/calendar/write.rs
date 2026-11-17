//! Writes weeks and exception days back into the `CLNDR_DATA` text P6 files hold.

use super::{CalendarData, Period};

/// P6 writes a line break inside a value as two DEL bytes; the scanner reads them as whitespace.
const BREAK: &str = "\u{7f}\u{7f}";

/// Writes one calendar as the text of a `CALENDAR.clndr_data` value.
///
/// The text has the shape [`parse_clndr_data`](super::parse_clndr_data) reads: a `CalendarData`
/// node holding `DaysOfWeek` (all seven days, Sunday first, written as nodes 1 through 7) when
/// the week is present, and `Exceptions` when there are any. Line breaks are DEL pairs with two
/// spaces of indent per level, as P6 writes them. A period finishing at minute 1440 is written
/// as `00:00`.
///
/// The text reads back to `data` when every period holds what the parser keeps: `start` below
/// `finish`, both inside the day, and no overlap. Each of an exception's `unparsed_periods` is
/// written as a bare period node with no `s` or `f`, which the parser counts as unreadable again
/// (and warns about), so an unreadable exception never becomes a whole-day holiday. Minutes past
/// the day are clamped to it rather than refused.
#[must_use]
pub fn write_clndr_data(data: &CalendarData) -> String {
    let mut children = Vec::new();
    if let Some(week) = &data.week {
        let days = week
            .iter()
            .enumerate()
            .map(|(index, periods)| {
                node(2, &(index + 1).to_string(), "", period_nodes(3, periods, 0))
            })
            .collect();
        children.push(node(1, "DaysOfWeek", "", days));
    }
    if !data.exceptions.is_empty() {
        let exceptions = data
            .exceptions
            .iter()
            .enumerate()
            .map(|(index, exception)| {
                let attrs = format!("d|{}", exception.serial);
                node(
                    2,
                    &index.to_string(),
                    &attrs,
                    period_nodes(3, &exception.periods, exception.unparsed_periods),
                )
            })
            .collect();
        children.push(node(1, "Exceptions", "", exceptions));
    }
    node(0, "CalendarData", "", children)
}

fn period_nodes(depth: usize, periods: &[Period], unparsed: usize) -> Vec<String> {
    let mut nodes: Vec<String> = periods
        .iter()
        .enumerate()
        .map(|(index, period)| {
            let attrs = format!(
                "s|{}|f|{}",
                clock(period.start.min(1_439)),
                clock(period.finish.min(1_440))
            );
            node(depth, &index.to_string(), &attrs, Vec::new())
        })
        .collect();
    for index in periods.len()..periods.len().saturating_add(unparsed) {
        nodes.push(node(depth, &index.to_string(), "", Vec::new()));
    }
    nodes
}

/// One node at `depth`: `(0||name(attrs)(children))`, led by a line break unless it is the root.
fn node(depth: usize, name: &str, attrs: &str, children: Vec<String>) -> String {
    let mut text = String::new();
    if depth > 0 {
        text.push_str(BREAK);
        text.push_str(&"  ".repeat(depth));
    }
    text.extend(["(0||", name, "(", attrs, ")("]);
    text.extend(children);
    text.push_str("))");
    text
}

/// Minutes since midnight as `HH:MM`; the end of the day is `00:00`.
fn clock(minutes: u16) -> String {
    let minutes = if minutes == 1_440 { 0 } else { minutes };
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
