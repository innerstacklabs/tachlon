//! The move report: after an edit that moves other tasks, how many moved and how the project
//! finish changed (gs#424, story S2 of #309). `status.moved` hands it to the page.
//!
//! An edit outside a press reports from the engine's moved list. A drag runs one edit per frame,
//! so the journal keeps the first old dates of each task across the frames, and the release
//! turns it into one report with the press-time positions.

mod critical;
mod journal;
mod slide;
mod text;

use crate::Session;
use engine::{DAY, Instant, Moved, TaskId};

/// A report stays fully visible this long, in milliseconds, then fades over [`FADE_MS`].
pub const HOLD_MS: f64 = 4000.0;
/// The fade after [`HOLD_MS`], in milliseconds.
pub const FADE_MS: f64 = 400.0;
/// After a hold ends (the pointer left the note), the report stays fully visible at least this
/// long, in milliseconds, whatever time it had left.
pub const RESUME_MIN_MS: f64 = 1000.0;

/// What one edit or drag moved besides the edited task.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveReport {
    /// Counts reports, so the page can tell a new one from the same one.
    pub seq: u32,
    /// `8 tasks moved · finish +7 days`.
    pub text: String,
    /// Tasks other than the edited one whose early start or finish changed.
    pub others: u32,
    /// Calendar days the project finish moved (negative: earlier).
    pub finish_days: i64,
    /// Tasks not critical (float above zero) before the edit and critical now; for a drag, before
    /// the press.
    pub joined: u32,
    /// Tasks critical before the edit and not now; for a drag, before the press.
    pub left: u32,
    /// True for an edit (the page may slide the bars), false for a drag release, whose bars
    /// are already where the pointer left them.
    pub slide: bool,
    /// Each moved task with the early dates it had before the edit (before the press, for a
    /// drag), in the order the engine listed them.
    pub was: Vec<Moved>,
    /// The edited task with the early dates it had before the edit (before the press, for a
    /// drag), or `None` when its early dates did not change. It is not in `was` and not counted
    /// in `others`.
    pub edited: Option<Moved>,
    /// The clock when the report was made.
    pub at: f64,
}

impl MoveReport {
    /// How opaque the report's note is `now`: 1 for [`HOLD_MS`], then down to 0 over [`FADE_MS`].
    /// With `reduced_motion` it is 1 for [`HOLD_MS`] and then 0, with no ramp. While `held` (the
    /// pointer is over the note) it is 1 whatever the age.
    #[must_use]
    pub fn alpha(&self, now: f64, held: bool, reduced_motion: bool) -> f64 {
        let age = now - self.at;
        if held || age <= HOLD_MS {
            1.0
        } else if reduced_motion {
            0.0
        } else {
            (1.0 - (age - HOLD_MS) / FADE_MS).clamp(0.0, 1.0)
        }
    }
}

/// The session's move report and what makes it.
#[derive(Default)]
pub(crate) struct Reports {
    pub(crate) report: Option<MoveReport>,
    /// The clock when the page said the pointer entered the note, while it is over it. The hold
    /// belongs to the session: a report that replaces the shown one keeps it; a change that
    /// leaves no report, and a reopened session, have none.
    pub(crate) held: Option<f64>,
    /// Counts the reports made; a reopened session carries it on.
    pub(crate) seq: u32,
    /// The critical flags taken before the edit that is running.
    critical: critical::Before,
    /// The moved row filter was on when the last change began: the report that change makes
    /// turns it back on (gs#703).
    pub(crate) reopen: bool,
    /// The slide state: the running slide, the last report seen, reduced motion.
    pub(crate) motion: slide::Motion,
    journal: Journal,
    /// What a data date change did to the date, kept for the report that change makes; the next
    /// report takes it.
    cause: Option<text::DataDate>,
}

/// The first old dates of every task a drag moved, kept across its frames.
#[derive(Default)]
struct Journal {
    on: bool,
    /// The dragged task: it is not an "other".
    task: u32,
    /// The project finish at the press.
    finish: Instant,
    /// The dragged task with its press-time dates, once a frame moved it.
    edited: Option<Moved>,
    /// Which tasks `entries` already holds, sized once at the press.
    seen: Vec<bool>,
    entries: Vec<Moved>,
}

impl Reports {
    /// Whether a drag journal is running, so the frames of one drag share one `reopen`.
    pub(crate) fn journal_on(&self) -> bool {
        self.journal.on
    }
}

impl Journal {
    fn begin(&mut self, task: u32, tasks: usize, finish: Instant) {
        self.on = true;
        self.task = task;
        self.finish = finish;
        self.edited = None;
        self.seen.clear();
        self.seen.resize(tasks, false);
        self.entries.clear();
        // Room for every task once, so a frame's merge never grows the vector.
        self.entries.reserve(tasks);
    }

    /// Add one frame's moved list: a task already held keeps its first old dates. The dragged
    /// task goes to `edited`, not `entries`. A task id past the press-time task count is skipped.
    fn merge(&mut self, list: &[Moved]) {
        for m in list {
            if m.task.0 == self.task && self.edited.is_none() {
                self.edited = Some(*m);
            }
            if let Some(seen) = self.seen.get_mut(m.task.0 as usize)
                && m.task.0 != self.task
                && !*seen
            {
                *seen = true;
                self.entries.push(*m);
            }
        }
    }
}

impl Session {
    /// Whether the moved filter is on.
    pub fn moved_only(&self) -> bool {
        self.row_filter == crate::RowFilter::Moved
    }

    /// Turn the moved filter on or off. False when turning it on with no move report and no held
    /// proposal, or with no outline or a bar press active.
    pub fn set_moved_only(&mut self, on: bool) -> bool {
        let wanted = if on {
            if self.reports.report.is_none() && self.proposal.is_none() {
                return false;
            }
            crate::RowFilter::Moved
        } else if self.row_filter == crate::RowFilter::Moved {
            crate::RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }

    /// The report of the last edit or drag, until another change replaces or clears it.
    pub fn move_report(&self) -> Option<&MoveReport> {
        self.reports.report.as_ref()
    }

    /// The tasks the moved filter keeps, by slot: the report's moved tasks and its edited one, and
    /// the tasks a held proposal would move; `None` while the filter is off. Read when the rows
    /// are rebuilt with it on.
    pub(crate) fn moved_members(&self) -> Option<Vec<bool>> {
        (self.row_filter == crate::RowFilter::Moved).then(|| {
            let mut members = vec![false; self.schedule.tasks().len()];
            let reported = (self.reports.report.iter()).flat_map(|r| r.was.iter().chain(&r.edited));
            let proposed = self
                .proposal
                .iter()
                .flat_map(crate::proposal::Proposal::tasks);
            for task in reported.map(|m| m.task.0).chain(proposed) {
                if let Some(slot) = members.get_mut(task as usize) {
                    *slot = true;
                }
            }
            members
        })
    }

    /// The held proposal changed or ended: the moved filter reads its tasks now. With no proposal
    /// and no report left to read, it turns off.
    pub(crate) fn follow_proposal_filter(&mut self) {
        if self.row_filter != crate::RowFilter::Moved {
            return;
        }
        if self.reports.report.is_none() && self.proposal.is_none() {
            self.row_filter = crate::RowFilter::Off;
        }
        if self.busy() {
            self.rows_stale = true;
        } else {
            self.apply_rows();
        }
    }

    /// The drag of `task` began (the pointer passed the threshold): start a journal for it.
    pub(crate) fn begin_journal(&mut self, task: u32) {
        let (n, finish) = (self.solution.dates.len(), self.solution.project_finish);
        // A `reopen` left by a change that made no report (a rename) must not outlive the filter
        // it was set for: the drag's release would turn the filter back on.
        if self.row_filter != crate::RowFilter::Moved {
            self.reports.reopen = false;
        }
        self.reports.journal.begin(task, n, finish);
        self.remember_critical_at_press();
    }

    /// The press ended: the journal is spent.
    pub(crate) fn end_journal(&mut self) {
        self.reports.journal.on = false;
    }

    /// `edited` was moved or resized and the engine listed what moved in `self.moved`;
    /// `old_finish` is the project finish before. Inside a press this feeds the journal.
    /// `held` is the hold the change that just cleared the report found: a report made here keeps
    /// it (an edit under the pointer replaces the note under the pointer).
    pub(crate) fn note_moved(&mut self, edited: TaskId, old_finish: Instant, held: Option<f64>) {
        if self.in_drag(edited) {
            self.reports.journal.merge(&self.moved);
            return;
        }
        let was = self.moved_others(self.moved.iter(), edited);
        let edited = self.moved_edited(&self.moved, edited);
        self.report_edit(was, edited, old_finish, held);
        self.settle_moved_filter();
    }

    /// The change that just ran made its report (or none): bring the moved filter back for it,
    /// so the rows read this report and not the one the change replaced. The filter stays off
    /// when the change made no report.
    pub(crate) fn settle_moved_filter(&mut self) {
        if !std::mem::take(&mut self.reports.reopen) || self.reports.report.is_none() {
            return;
        }
        self.row_filter = crate::RowFilter::Moved;
        self.follow_filter();
    }

    /// A link edit changed the solution and `self.moved` lists every task whose early dates
    /// differ. No task is the edited one, so all of them are counted, outlined and slid, with the
    /// note, the hold and the slide of [`Session::note_moved`].
    pub(crate) fn note_links_moved(&mut self, old_finish: Instant, held: Option<f64>) {
        let was = self.moved.clone();
        self.report_edit(was, None, old_finish, held);
        self.settle_moved_filter();
    }

    /// The data date went from `was` to `now`: the report the change makes names it. Calendar
    /// days, as the finish is counted.
    pub(crate) fn note_data_date(&mut self, was: Option<Instant>, now: Option<Instant>) {
        self.reports.cause = match (was, now) {
            (Some(a), Some(b)) => Some(text::DataDate::Shifted(
                b.div_euclid(DAY) - a.div_euclid(DAY),
            )),
            (Some(_), None) => Some(text::DataDate::Cleared),
            (None, Some(_)) => Some(text::DataDate::Set),
            (None, None) => None,
        };
    }

    /// The report of an edit outside a press, with the hold the replaced note had.
    fn report_edit(
        &mut self,
        was: Vec<Moved>,
        edited: Option<Moved>,
        old_finish: Instant,
        held: Option<f64>,
    ) {
        let critical = self.take_critical_change(!was.is_empty());
        self.make_report(was, edited, old_finish, true, critical);
        if self.reports.report.is_some() {
            self.reports.held = held;
        }
    }

    fn make_report(
        &mut self,
        was: Vec<Moved>,
        edited: Option<Moved>,
        old_finish: Instant,
        slide: bool,
        (joined, left): (u32, u32),
    ) {
        let cause = self.reports.cause.take();
        if was.is_empty() {
            return;
        }
        let day = |t: Instant| self.origin.axis().day() + t.div_euclid(DAY);
        let finish_days = day(self.solution.project_finish) - day(old_finish);
        let others = u32::try_from(was.len()).unwrap_or(u32::MAX);
        self.reports.seq = self.reports.seq.wrapping_add(1);
        self.reports.report = Some(MoveReport {
            seq: self.reports.seq,
            text: text::summary(&self.texts, cause, others, finish_days, joined, left),
            others,
            finish_days,
            joined,
            left,
            slide,
            was,
            edited,
            at: (self.clock)(),
        });
    }

    /// How opaque `report` is now: full while the pointer holds the note.
    fn moved_alpha(&self, report: &MoveReport) -> f64 {
        let held = self.reports.held.is_some();
        report.alpha((self.clock)(), held, self.reports.motion.reduced_motion)
    }

    /// The page says the pointer is over the note (`on`) or has left it. While held the report
    /// stays fully visible, and so do its outlines. A release keeps the time the report had left,
    /// and at least [`RESUME_MIN_MS`] of it, before the fade. A hold with no report, or after the
    /// report has faded, does nothing; so does a repeat of the state the session is in.
    pub fn set_moved_hold(&mut self, on: bool) {
        let now = (self.clock)();
        let reduced = self.reports.motion.reduced_motion;
        let Some(report) = &mut self.reports.report else {
            return;
        };
        match (on, self.reports.held) {
            (true, None) if report.alpha(now, false, reduced) > 0.0 => {
                self.reports.held = Some(now);
            }
            (false, Some(since)) => {
                // A report made after the hold began is fresh: it has all of [`HOLD_MS`] left.
                let aged = (since - report.at).clamp(0.0, HOLD_MS);
                let left = (HOLD_MS - aged).max(RESUME_MIN_MS);
                report.at = now - (HOLD_MS - left);
                self.reports.held = None;
            }
            _ => {}
        }
    }

    /// Add the dashed outlines of the move report to `scene`, at the old places, while the report
    /// is visible. Draw only: nothing here touches the `Solution`.
    pub fn add_move_outlines(&self, scene: &mut timeline::Scene) {
        let Some(report) = &self.reports.report else {
            return;
        };
        let tl = self.timeline();
        timeline::scene::add_move_outlines(
            scene,
            &tl.viewport,
            tl.dpr,
            tl.summaries(),
            |task| tl.row_of_task(task),
            report.was.iter().map(|m| (m.task.0, m.was_es, m.was_ef)),
            self.moved_alpha(report),
            self.theme(),
        );
    }

    /// The `moved` value of `status_json`: `null` or the report with its current alpha.
    pub(crate) fn moved_json(&self) -> String {
        match &self.reports.report {
            None => "null".to_string(),
            Some(r) => format!(
                r#"{{"seq":{},"text":{},"others":{},"finish_days":{},"alpha":{:.3},"slide":{}}}"#,
                r.seq,
                crate::quote(&r.text),
                r.others,
                r.finish_days,
                self.moved_alpha(r),
                r.slide
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved(task: u32, at: Instant) -> Moved {
        Moved {
            task: TaskId(task),
            was_es: at,
            was_ef: at + 1,
        }
    }

    #[test]
    fn a_merge_allocates_nothing_and_skips_an_unknown_task() {
        let mut j = Journal::default();
        j.begin(0, 5, 0);
        let cap = j.entries.capacity();
        assert!(cap >= 5);
        let first: Vec<Moved> = (0..5).map(|t| moved(t, 10)).collect();
        j.merge(&first);
        j.merge(&[moved(1, 99), moved(9, 99)]);
        assert_eq!(
            j.entries.capacity(),
            cap,
            "a frame's merge grew the entries"
        );
        let tasks: Vec<u32> = j.entries.iter().map(|m| m.task.0).collect();
        assert_eq!(tasks, [1, 2, 3, 4], "edited task and unknown task skipped");
        assert!(j.entries.iter().all(|m| m.was_es == 10), "first dates kept");
    }
}
