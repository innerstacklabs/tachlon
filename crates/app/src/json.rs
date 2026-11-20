//! The JSON the page reads (gs#254, split out of `web.rs`): the status line, the task table's
//! rows and the hover card, as `impl Session`. They are plain Rust, so a native test builds a
//! real [`Session`] and reads the keys they write (`tests/json_contract`); `packages/gantt/src/api.ts` types
//! the same shapes by hand. Every string goes through [`quote`], which escapes the way
//! `serde_json` does.

use crate::{Session, lookahead};
use engine::{Instant, TaskId};
use std::fmt::Write as _;
use timeline::axis;

/// What `status_json` reads from outside the session: the renderer and the last frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderFacts {
    /// "WebGPU" or "WebGL2".
    pub backend: &'static str,
    /// The surface size in physical pixels.
    pub surface: (u32, u32),
    pub bars: usize,
    pub critical_bars: usize,
    pub arrows: usize,
    /// Non-working-day columns the last frame drew.
    pub shaded: usize,
    /// x of the data-date line in CSS pixels, if the last frame drew it.
    pub data_date_x: Option<f64>,
    /// Files opened in place; the shell resets its own state when it moves.
    pub epoch: u32,
    /// Frames drawn.
    pub frames: u64,
}

/// `text` as a JSON string literal, with `"`, `\` and control characters escaped as
/// `serde_json` writes them.
#[must_use]
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON number, or `null` for `None`.
fn number<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

/// A JSON array of strings.
pub(crate) fn string_array(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|v| quote(v)).collect();
    format!("[{}]", items.join(","))
}

impl Session {
    /// A summary task's rolled-up `(start, finish)`, or `None` for a leaf.
    pub fn summary_span(&self, task: u32) -> Option<(Instant, Instant)> {
        let outline = self.outline()?;
        if !outline.is_summary(TaskId(task)) {
            return None;
        }
        let dates = &self.solution().dates[task as usize];
        Some(
            self.timeline()
                .summaries()
                .get(task)
                .unwrap_or((dates.es, dates.ef)),
        )
    }

    /// The task table's rows `first..last` (clamped to the schedule) as a JSON array of
    /// `[task, start, finish, float in minutes or null, critical, kind, start variance, finish
    /// variance, float variance]`, where the float is `null` for a summary and for a finished task
    /// (the table shows no float cell for it), and kind is 0 for a leaf, 1 for an expanded summary, and 2 for
    /// a collapsed summary. While grouped a header row is `[null, value or null for no value, leaf
    /// count, null, false, 3, null, null, null]`, with kind 4 while its group is collapsed. The variances are text (working days, as `variance_cells` writes them),
    /// `null` for a task with no baseline entry, with no baseline and while a bar is being dragged;
    /// the float one is also `null` for a summary and for a baseline a file saved, which has no
    /// float, and a start or finish one when the baseline's date is before the origin. A summary's
    /// start and finish read its rolled-up span against the baseline's (`summary_variance_cells`).
    pub fn table_rows_json(&self, first: u32, last: u32) -> String {
        let order = self.timeline().rows();
        let end = (last as usize).min(order.len());
        let start = (first as usize).min(end);
        let rows: Vec<String> = (start..end)
            .map(|row| {
                let t = order[row];
                if let Some((value, count, collapsed)) = self.header(row) {
                    let label = value.map_or_else(|| "null".to_string(), quote);
                    let kind = if collapsed { 4 } else { 3 };
                    return format!("[null,{label},{count},null,false,{kind},null,null,null]");
                }
                let d = &self.solution().dates[t as usize];
                // The page holds the columns while a bar is dragged, so a drag frame does not compute them.
                let held = self.drag_started();
                let cells_json = |cells: Option<[Option<String>; 3]>| match cells {
                    Some(cells) => {
                        cells.map(|text| text.map_or_else(|| "null".to_string(), |t| quote(&t)))
                    }
                    None => ["null", "null", "null"].map(str::to_string),
                };
                if let Some((start, finish)) = self.summary_span(t) {
                    let kind = if self.is_collapsed(TaskId(t)) { 2 } else { 1 };
                    let cells = if held {
                        None
                    } else {
                        self.summary_variance_cells(TaskId(t), (start, finish))
                    };
                    let [vs, vf, vl] = cells_json(cells);
                    format!(
                        "[{t},{},{},null,false,{kind},{vs},{vf},{vl}]",
                        quote(&self.date_text(start)),
                        quote(&self.date_text(finish))
                    )
                } else {
                    let cells = if held {
                        None
                    } else {
                        self.variance_cells(TaskId(t))
                    };
                    let [vs, vf, vl] = cells_json(cells);
                    let float = if d.finished {
                        "null".to_string()
                    } else {
                        d.float.to_string()
                    };
                    format!(
                        "[{t},{},{},{float},{},0,{vs},{vf},{vl}]",
                        quote(&self.date_text(d.es)),
                        quote(&self.date_text(d.ef)),
                        d.is_critical()
                    )
                }
            })
            .collect();
        format!("[{}]", rows.join(","))
    }

    /// Everything the hover card shows for `task` as a JSON object, or `None` past the schedule.
    pub fn task_details_json(&self, task: u32) -> Option<String> {
        let d = self.task_details(task)?;
        let dates = match self.summary_span(task) {
            Some((start, finish)) => format!(
                r#""start":{},"finish":{},"summary":true,"#,
                quote(&self.date_text(start)),
                quote(&self.date_text(finish))
            ),
            None => format!(
                r#""start":{},"finish":{},"#,
                quote(&d.start),
                quote(&d.finish)
            ),
        };
        // The baseline keys are written only for a leaf with an entry; a date or variance the file's
        // baseline cannot give is `null` (variance) or left out (date).
        let mut baseline = String::new();
        if let Some([first, second]) = &d.baseline_variance {
            for (key, date) in [
                ("baselineStart", &d.baseline_start),
                ("baselineFinish", &d.baseline_finish),
            ] {
                if let Some(date) = date {
                    let _ = write!(baseline, r#","{key}":{}"#, quote(date));
                }
            }
            let text = |v: &Option<String>| v.as_deref().map_or_else(|| "null".to_string(), quote);
            let _ = write!(
                baseline,
                r#","baselineVariance":[{},{}]"#,
                text(first),
                text(second)
            );
        }
        Some(format!(
            concat!(
                r#"{{"task":{},"name":{},"outlinePath":{},"activityId":{},"status":{},"#,
                r#""actualStart":{},"actualFinish":{},"percentComplete":{},{}"lateStart":{},"lateFinish":{},"#,
                r#""floatMinutes":{},"floatDays":{},"floatHours":{},"floatRemMinutes":{},"#,
                r#""floatText":{},"critical":{},"predecessorCount":{},"predecessors":{},"#,
                r#""successorCount":{},"successors":{},"resources":{}{}}}"#
            ),
            d.task,
            quote(&d.name),
            quote(&d.outline_path),
            quote(&d.activity_id),
            quote(&d.status),
            quote(&d.actual_start),
            quote(&d.actual_finish),
            quote(&d.percent_complete),
            dates,
            quote(&d.late_start),
            quote(&d.late_finish),
            d.float_minutes,
            d.float_days,
            d.float_hours,
            d.float_rem_minutes,
            quote(&d.float_text),
            d.critical,
            d.predecessor_count,
            string_array(&d.predecessors),
            d.successor_count,
            string_array(&d.successors),
            string_array(&d.resources),
            baseline,
        ))
    }

    /// The solved start of every live task as a JSON array of `YYYY-MM-DD HH:MM` texts, in the order
    /// `document::write` lists the tasks (a summary's is the start of its span): one read for what
    /// [`Session::task_details_json`] gives a task at a time.
    pub fn task_starts_json(&self) -> String {
        let starts: Vec<String> = crate::document::live_order(self)
            .iter()
            .map(|task| {
                let start = self.summary_span(task.0).map_or_else(
                    || self.solution().dates[task.0 as usize].es,
                    |(start, _)| start,
                );
                quote(&self.date_text(start))
            })
            .collect();
        format!("[{}]", starts.join(","))
    }

    /// State for the overlay and for tests, as a JSON object.
    #[expect(clippy::too_many_lines, reason = "builds the status field by field")]
    pub fn status_json(&self, facts: &RenderFacts) -> String {
        let v = &self.timeline().viewport;
        let rows = v.visible_rows();
        let (t_from, t_to) = v.visible_time();
        let (minor, _) = axis::levels_for(v.px_per_minute);
        let origin = self.origin();
        let when = |t: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "t is a visible time in minutes; a float-to-int `as` saturates, so a value outside i64 clamps and NaN becomes 0, and it never wraps"
            )]
            let t = t.floor() as i64;
            format!(
                "{} {:02}:{:02}",
                axis::Level::Day.long_label(origin.axis(), t, &self.names),
                t.rem_euclid(1440) / 60,
                t.rem_euclid(60)
            )
        };
        let hover = match self.timeline().hovered(&self.solution().dates) {
            Some(t) => {
                let d = &self.solution().dates[t as usize];
                if let Some((start, finish)) = self.summary_span(t) {
                    format!(
                        r#"{{"task":{t},"es":{start},"ef":{finish},"critical":false,"summary":true}}"#
                    )
                } else {
                    // `edge`: the pointer is on the bar's finish edge, where a drag sets the duration. A finished
                    // task's press is no resize (it is fixed at its actuals), so its edge is not offered.
                    let edge = if self.timeline().hovered_edge(&self.solution().dates)
                        && !self.is_finished(TaskId(t))
                    {
                        r#","edge":true"#
                    } else {
                        ""
                    };
                    format!(
                        r#"{{"task":{t},"es":{},"ef":{},"float":{},"critical":{}{edge}}}"#,
                        d.es,
                        d.ef,
                        d.float,
                        d.is_critical()
                    )
                }
            }
            None => "null".to_string(),
        };
        let drag = match self.drag_info() {
            Some(d) => format!(
                r#"{{"task":{},"wanted":{},"es":{},"held":{},"recomputeMs":{:.3},"active":{}}}"#,
                d.task,
                d.wanted,
                d.es,
                d.held,
                d.recompute_ms,
                self.drag_started()
            ),
            None => "null".to_string(),
        };
        let selected = match self.selection() {
            Some(s) => format!(
                concat!(
                    r#"{{"task":{},"row":{},"backCount":{},"forwardCount":{},"backCapped":{},"forwardCapped":{}}}"#
                ),
                s.task,
                s.row,
                s.back.len(),
                s.forward.len(),
                s.back_capped,
                s.forward_capped
            ),
            None => "null".to_string(),
        };
        // The window that is set, or the one a reader would get: the data date's day for
        // 3 weeks (the project origin's day when the file has no data date).
        let window = self.lookahead();
        let lookahead = format!(
            r#"{{"on":{},"start":{},"weeks":{}}}"#,
            window.is_some(),
            quote(&self.lookahead_start_text()),
            window.map_or(lookahead::DEFAULT_WEEKS, |w| w.weeks)
        );
        format!(
            concat!(
                r#"{{"backend":{},"n":{},"rowCount":{},"keptLeaves":{},"path":{},"criticalOnly":{},"longestPath":{},"filter":{},"rows":[{},{}],"t":[{},{}],"from":{},"to":{},"#,
                r#""level":{},"pxPerMinute":{},"scrollY":{},"rowHeight":{},"axisHeight":{},"bars":{},"criticalBars":{},"#,
                r#""arrows":{},"shaded":{},"dataDate":{},"dataDateX":{},"lookahead":{},"revision":{},"stamp":{},"epoch":{},"frames":{},"surface":[{},{}],"hover":{},"hoverRow":{},"drag":{},"selected":{},"range":{},"link":{},"gesture":{},"offscreen":{},"baseline":{},"moved":{},"slide":{}}}"#
            ),
            quote(facts.backend),
            self.task_count(),
            self.timeline().rows().len(),
            number(self.kept_leaves),
            self.path_line_json(),
            self.critical_only(),
            self.longest_path(),
            quote(self.row_filter().name()),
            rows.start,
            rows.end,
            t_from,
            t_to,
            quote(&when(t_from)),
            quote(&when(t_to)),
            quote(minor.name()),
            v.px_per_minute,
            v.scroll_y,
            v.row_height,
            v.axis_height,
            facts.bars,
            facts.critical_bars,
            facts.arrows,
            facts.shaded,
            number(self.data_date()),
            number(facts.data_date_x),
            lookahead,
            self.revision(),
            self.stamp(),
            facts.epoch,
            facts.frames,
            facts.surface.0,
            facts.surface.1,
            hover,
            number(self.timeline().hover_row()),
            drag,
            selected,
            self.range_json(),
            self.link_status_json(),
            quote(self.gesture().name()),
            self.offscreen_json(),
            self.baseline_json(),
            self.moved_json(),
            self.slide_json()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quote_writes_what_serde_json_writes() {
        for text in [
            "",
            "plain",
            r#"say "hi""#,
            r"back\slash",
            r#"a\"b\\"#,
            "line\nbreak\ttab\r",
            "\u{0}\u{1}\u{8}\u{c}\u{1f}\u{7f}",
            "Pour slab / Zürich / 日本語 / 🏗",
        ] {
            assert_eq!(
                quote(text),
                serde_json::to_string(text).unwrap(),
                "{text:?}"
            );
        }
    }
}
