//! The slide state (gs#484, story S3.4 of gs#427): the one timed copy of the dates a slide draws,
//! the snap rules, and reduced motion.
//!
//! The solution is never blended. A slide draws from a scratch copy of `solution.dates` that
//! [`timeline::slide::apply`] writes the moved tasks into on every frame; [`Session::display_dates`]
//! hands that copy to a drawing consumer while a slide runs and `solution.dates` itself otherwise.
//! Nothing here calls the engine or writes `solution`.
//!
//! One step, [`Session::advance_motion`], does all of it, once per frame, before the frame draws:
//!
//! 1. A slide that was started on another [`Session::stamp`] snaps. The stamp goes up on every new
//!    solution, so this one rule covers an edit, an undo, a redo, an open and the edits that make
//!    no report (insert, delete, link, reparent): the copy would hold dates from before them.
//! 2. A report whose `seq` is new is marked seen, and starts a slide if it asks for one (`slide`),
//!    reduced motion is off and a moved task has a row. A report that does not start one still
//!    counts as seen, so it never plays later.
//! 3. A running slide takes its time from the session clock and ends when its total has passed.
//!
//! The slide starts on the first frame that sees the report, not when the report was made, so a
//! slow edit does not use up the slide's time.

use super::MoveReport;
use crate::Session;
use engine::{Moved, TaskDates};
use timeline::slide;

/// A running slide: the entries it moves and the scratch copy it draws.
pub(crate) struct Slide {
    /// The session `stamp` when the slide began; another stamp snaps it.
    stamp: u32,
    /// The clock when the slide began, in milliseconds.
    started: f64,
    /// When the slide ends, in milliseconds after it began.
    total: f64,
    /// The moved tasks that have a row, sorted by old start.
    entries: Vec<Moved>,
    /// `solution.dates` as it was when the slide began, with the entries blended.
    copy: Vec<TaskDates>,
}

/// What the session keeps for the slide.
#[derive(Default)]
pub(crate) struct Motion {
    /// The running slide, if any.
    pub(crate) slide: Option<Slide>,
    /// The `seq` of the last report a frame has seen.
    pub(crate) seen: u32,
    /// The page asked for no motion: no report starts a slide.
    #[expect(
        clippy::struct_field_names,
        reason = "the name is the page's own setting, as in `set_reduced_motion`"
    )]
    pub(crate) reduced_motion: bool,
    /// How long the last copy took, in milliseconds; `None` before the first slide.
    pub(crate) copy_ms: Option<f64>,
    /// The Fit or find flight a frame has not finished (gs#309); see `fly.rs`.
    pub(crate) fly: Option<crate::fly::Fly>,
}

impl Session {
    /// The dates to draw: the slide's copy while one runs, else `solution().dates` itself (the same
    /// slice, no copy). The solution is not changed by a slide.
    pub fn display_dates(&self) -> &[TaskDates] {
        match &self.reports.motion.slide {
            Some(slide) => &slide.copy,
            None => &self.solution.dates,
        }
    }

    /// Whether the page asked for no motion. On, a report never starts a slide, and one made while
    /// it was on never plays after it is turned off. A change drops the slide that runs.
    pub fn set_reduced_motion(&mut self, on: bool) {
        let motion = &mut self.reports.motion;
        if motion.reduced_motion == on {
            return;
        }
        motion.reduced_motion = on;
        motion.slide = None;
        if let Some(report) = &self.reports.report {
            motion.seen = report.seq;
        }
    }

    /// How long the copy of the last slide took, in milliseconds, on the session clock; `None`
    /// before the first slide.
    pub fn slide_copy_ms(&self) -> Option<f64> {
        self.reports.motion.copy_ms
    }

    /// Whether a slide runs: [`Session::display_dates`] hands out the copy.
    pub fn slide_active(&self) -> bool {
        self.reports.motion.slide.is_some()
    }

    /// The slide as the page reads it in `status`: `{"active":bool,"copyMs":number|null}`. The
    /// copy time stays after the slide ends, and is `null` before the first one.
    pub(crate) fn slide_json(&self) -> String {
        let copy_ms = self
            .slide_copy_ms()
            .map_or_else(|| "null".to_string(), |ms| format!("{ms:.3}"));
        format!(r#"{{"active":{},"copyMs":{copy_ms}}}"#, self.slide_active())
    }

    /// One frame of motion: snap a slide of another stamp, start one for a new report, step the
    /// one that runs. Cheap and allocation-free on an idle frame. A held proposal is solved again
    /// first when an edit changed the schedule (`refresh_proposal`).
    pub fn advance_motion(&mut self) {
        self.refresh_proposal();
        let stamp = self.stamp;
        if self
            .reports
            .motion
            .slide
            .as_ref()
            .is_some_and(|slide| slide.stamp != stamp)
        {
            self.reports.motion.slide = None;
        }
        if self
            .reports
            .report
            .as_ref()
            .is_some_and(|report| report.seq != self.reports.motion.seen)
        {
            self.start_slide();
        }
        let Some(slide) = &mut self.reports.motion.slide else {
            return;
        };
        let t = (self.clock)() - slide.started;
        if t >= slide.total {
            self.reports.motion.slide = None;
        } else {
            timeline::slide::apply(&slide.entries, &self.solution.dates, t, &mut slide.copy);
        }
    }

    /// The report has a `seq` no frame has seen: mark it seen, and start its slide if it has one.
    fn start_slide(&mut self) {
        let Some(report) = &self.reports.report else {
            return;
        };
        let motion = &mut self.reports.motion;
        motion.seen = report.seq;
        if !report.slide || motion.reduced_motion {
            return;
        }
        let entries = entries_with_a_row(report, |task| self.timeline.row_of_task(task).is_some());
        if entries.is_empty() {
            return;
        }
        self.begin_slide(entries);
    }

    /// Start a slide of `entries` (sorted by old start) now, over a copy of the dates.
    fn begin_slide(&mut self, entries: Vec<Moved>) {
        let started = (self.clock)();
        let copy = self.solution.dates.clone();
        let motion = &mut self.reports.motion;
        motion.copy_ms = Some((self.clock)() - started);
        motion.slide = Some(Slide {
            stamp: self.stamp,
            started,
            total: slide::total_ms(entries.len()),
            entries,
            copy,
        });
    }

    /// Start the slide of a file open: each of `entries` is drawn from `was_es`/`was_ef` to its
    /// solved dates over the usual slide. The copy already holds the first frame, so
    /// [`Session::display_dates`] shows it before any frame runs. Nothing starts under reduced
    /// motion or for no entries. The entries need not be sorted.
    pub fn start_slide_from(&mut self, mut entries: Vec<Moved>) {
        if self.reports.motion.reduced_motion || entries.is_empty() {
            return;
        }
        slide::sort_by_old_start(&mut entries);
        self.begin_slide(entries);
        if let Some(slide) = &mut self.reports.motion.slide {
            timeline::slide::apply(&slide.entries, &self.solution.dates, 0.0, &mut slide.copy);
        }
    }

    /// A scroll, zoom or Fit ends a running slide at once: the bars are at their solved dates.
    pub(crate) fn snap_slide(&mut self) {
        self.reports.motion.slide = None;
    }

    /// Mark a file open (gs#309): the bars of the rows in the window grow from their start to
    /// their finish, staggered by start date. Only those rows are entered, so the cost is bounded
    /// by the window and not by the schedule. A scroll, zoom or edit snaps it (the stamp rule).
    pub fn mark_open(&mut self) {
        let rows = self.timeline.viewport.visible_rows();
        let order = self.timeline.order();
        let outline = self.outline.as_ref();
        let entries = order[rows.start.min(order.len())..rows.end.min(order.len())]
            .iter()
            .filter_map(|&task| {
                if outline.is_some_and(|o| o.is_summary(engine::TaskId(task))) {
                    return None;
                }
                let d = self.solution.dates.get(task as usize)?;
                Some(Moved {
                    task: engine::TaskId(task),
                    was_es: d.es,
                    was_ef: d.es,
                })
            })
            .collect();
        self.start_slide_from(entries);
    }
}

/// The edited task and the others of `report` that `has_row` accepts, sorted by old start. A task
/// with no row is not drawn, so it is not slid.
fn entries_with_a_row(report: &MoveReport, has_row: impl Fn(u32) -> bool) -> Vec<Moved> {
    let mut entries: Vec<Moved> = report
        .edited
        .iter()
        .chain(&report.was)
        .filter(|m| has_row(m.task.0))
        .copied()
        .collect();
    slide::sort_by_old_start(&mut entries);
    entries
}

impl super::Reports {
    /// What a reopened session keeps of this one: the report counter, so a page cannot take a new
    /// report for one it has shown, and the page's reduced-motion setting. The slide, the last
    /// `seq` seen and the copy time start over.
    pub(crate) fn carry_on(&mut self, old: &Self) {
        self.seq = old.seq;
        self.motion.reduced_motion = old.motion.reduced_motion;
    }
}
