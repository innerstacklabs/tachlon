//! The baseline summary: the counts the status line reports, and the memo that keeps the last one.

use super::Baseline;
use crate::Session;
use crate::edit::{day_minutes, format_float_days};
use engine::{DAY, Instant, Solution, TaskDates, TaskId};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use timeline::Summaries;

/// How the schedule now differs from a [`Baseline`], in the counts the status line reports. Tasks
/// are matched by id; a task counts only if it is live now and was live when the baseline was set,
/// except for `added` and `removed`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BaselineSummary {
    /// Tasks whose early finish is later than the baseline's.
    pub slipped: u32,
    /// Calendar days the project finish moved; negative when earlier.
    pub finish_days: i64,
    /// Tasks that are critical now (zero float, not finished) and had float in the baseline.
    pub joined: u32,
    /// Tasks that had zero float in the baseline and are not critical now, finished tasks
    /// excepted: a finished task is never critical, so it is not counted as leaving the path.
    pub left: u32,
    /// Live tasks the baseline has no entry for (inserted since).
    pub added: u32,
    /// Tasks the baseline holds that are deleted now, and, for a baseline read from an older file,
    /// the tasks only that file has.
    pub removed: u32,
}

/// The longest [`BaselineSummary::json`] can be: its keys and punctuation (65 bytes) and six
/// numbers, five `u32` (10 digits) and one `i64` (19 digits and a sign) among them.
pub const SUMMARY_JSON_MAX: usize = 135;

impl BaselineSummary {
    /// The `status.baseline` object, at most [`SUMMARY_JSON_MAX`] bytes.
    #[must_use]
    pub fn json(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for BaselineSummary {
    /// The `status.baseline` object, written straight into the caller's buffer.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            r#"{{"slipped":{},"finish_days":{},"joined":{},"left":{},"added":{},"removed":{}}}"#,
            self.slipped, self.finish_days, self.joined, self.left, self.added, self.removed
        )
    }
}

/// The `baseline` value of `status_json`, written without an intermediate `String`: `null` with
/// no baseline, else the summary's object.
pub(crate) struct BaselineJson(pub(super) Option<BaselineSummary>);

impl fmt::Display for BaselineJson {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(summary) => summary.fmt(out),
            None => out.write_str("null"),
        }
    }
}

/// The fewest milliseconds between two computations of the summary while a bar is being dragged.
/// A drag changes the solution every frame and the summary reads all of it, so the line keeps the
/// last value in between; the release and any read at rest compute the exact one.
pub const DRAG_SUMMARY_INTERVAL_MS: f64 = 150.0;

/// A summary kept with the key it was computed for and the clock when it was.
#[derive(Debug, Clone, Copy)]
pub(super) struct Kept {
    key: (u32, u32),
    at: f64,
    summary: BaselineSummary,
}

/// The summary for one `(revision, stamp)` of the session.
#[derive(Debug, Default)]
pub(super) struct Memo {
    cached: Mutex<Option<Kept>>,
    pub(super) runs: AtomicU32,
    /// The snapshot as a [`Solution`], built on the first read and kept: it never changes.
    pub(super) solution: OnceLock<Solution>,
    /// The spans of the baseline's own dates rolled up over the outline in force, with the
    /// `(revision, stamp)` they were built for. A structure edit moves the key.
    spans: Mutex<Option<((u32, u32), Summaries)>>,
}

impl Clone for Memo {
    fn clone(&self) -> Memo {
        Memo::default()
    }
}

impl PartialEq for Memo {
    fn eq(&self, _: &Memo) -> bool {
        true
    }
}

impl Eq for Memo {}

impl Memo {
    /// The summary kept for `key`. Else, in a drag, the one kept before when it is younger than
    /// [`DRAG_SUMMARY_INTERVAL_MS`] by `clock` (the key stays the old one, so the next read that is
    /// due or at rest computes). Else `compute()` (once) and keep it.
    pub(super) fn get(
        &self,
        key: (u32, u32),
        dragging: bool,
        clock: fn() -> f64,
        compute: impl FnOnce() -> BaselineSummary,
    ) -> BaselineSummary {
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(kept) = *cached {
            if kept.key == key {
                return kept.summary;
            }
            // A clock that went backwards or is not a number is not young: compute.
            let age = clock() - kept.at;
            if dragging && (0.0..DRAG_SUMMARY_INTERVAL_MS).contains(&age) {
                return kept.summary;
            }
        }
        self.runs.fetch_add(1, Ordering::Relaxed);
        let at = clock();
        let summary = compute();
        *cached = Some(Kept { key, at, summary });
        summary
    }
}
impl Memo {
    /// The baseline's rolled-up span of `task` for `key`, building the spans with `build` when the
    /// key has moved. `None` when none of the summary's tasks is in the baseline.
    fn span(
        &self,
        key: (u32, u32),
        task: TaskId,
        build: impl FnOnce() -> Summaries,
    ) -> Option<(Instant, Instant)> {
        let mut spans = self.spans.lock().unwrap_or_else(PoisonError::into_inner);
        if spans.as_ref().is_none_or(|(kept, _)| *kept != key) {
            *spans = Some((key, build()));
        }
        let (es, ef) = spans.as_ref()?.1.get(task.0)?;
        (es <= ef).then_some((es, ef))
    }
}

impl Session {
    /// A summary row's Start and Finish variance in working days of its own calendar (the text
    /// `variance_cells` writes for a leaf), read from its rolled-up span `now` against the span of
    /// the baseline's own dates rolled up over the same outline. A task the baseline has no entry
    /// for is left out of that span, as is one deleted since. Float is `None`: a summary has none.
    /// `None` with no baseline or entry, and when no task of the summary is in the baseline.
    ///
    /// The spans are built once per `(revision, stamp)`, not per call, so a frame costs one lookup
    /// per visible summary row.
    pub(crate) fn summary_variance_cells(
        &self,
        task: TaskId,
        now: (Instant, Instant),
    ) -> Option<[Option<String>; 3]> {
        let baseline = self.baseline.as_ref()?;
        baseline.task(task)?;
        let outline = self.outline()?;
        let key = (self.revision(), self.stamp());
        let was = baseline.memo.span(key, task, || {
            // A task the baseline lacks gets the identity of min and max, so it joins no span.
            let absent = TaskDates {
                es: Instant::MAX,
                ef: Instant::MIN,
                ls: 0,
                lf: 0,
                float: 0,
                finished: false,
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let dates: Vec<TaskDates> = (0..outline.len())
                .map(|i| {
                    baseline
                        .task(TaskId(i as u32))
                        .map_or(absent, |t| TaskDates {
                            es: t.es,
                            ef: t.ef,
                            ..absent
                        })
                })
                .collect();
            let mut spans = Summaries::new(outline.len());
            outline.rollup_full(&dates, &mut spans);
            spans
        })?;
        let calendar = self.schedule.calendar_of(task);
        let day = day_minutes(calendar);
        let moved = |now: Instant, was: Instant| {
            let change = calendar.count(now) - calendar.count(was);
            match change {
                0 => "0".to_string(),
                1.. => format!("+{}", format_float_days(change, day)),
                _ => format_float_days(change, day),
            }
        };
        // `Calendar::count` needs an instant on or after the origin, as for a leaf.
        let cell = |now: Instant, was: Instant| (was >= 0).then(|| moved(now, was));
        Some([cell(now.0, was.0), cell(now.1, was.1), None])
    }

    /// The summary from [`engine::diff`] of the baseline and the current solution, with the
    /// engine's slot-for-slot view corrected for tasks that are deleted now or had no entry then.
    pub(crate) fn summarize(&self, baseline: &Baseline) -> BaselineSummary {
        let now = &self.solution;
        let diff = engine::diff(baseline.as_solution(), now);
        let live_in_both =
            |task: &TaskId| baseline.task(*task).is_some() && !self.schedule.is_deleted(*task);
        // A baseline from a file has dates, not float: such a task joins or leaves in no count.
        let has_float = |task: &TaskId| baseline.task(*task).is_some_and(|t| t.float.is_some());
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let slipped = diff
            .changes
            .iter()
            .filter(|c| live_in_both(&c.task) && c.finish > 0)
            .count();
        let (mut added, mut removed) = (0u32, 0u32);
        for i in 0..self.schedule.tasks().len().max(baseline.tasks.len()) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let task = TaskId(i as u32);
            let live = i < self.schedule.tasks().len() && !self.schedule.is_deleted(task);
            match (live, baseline.task(task).is_some()) {
                (true, false) => added += 1,
                (false, true) => removed += 1,
                _ => {}
            }
        }
        let day = |t: Instant| t.div_euclid(DAY);
        BaselineSummary {
            slipped: count(slipped),
            finish_days: day(now.project_finish) - day(baseline.project_finish),
            joined: count(
                diff.joined_critical
                    .iter()
                    .filter(|t| live_in_both(t) && has_float(t))
                    .count(),
            ),
            // The snapshot keeps float, not progress, so a task finished now with zero float in
            // the baseline cannot be told from one that was critical then and is done since.
            // Finished work is never critical, so it does not count as leaving the path.
            left: count(
                diff.left_critical
                    .iter()
                    .filter(|t| live_in_both(t) && has_float(t) && !now.get(**t).finished)
                    .count(),
            ),
            added,
            // A task only in the older file has no slot here, so the loop above cannot see it.
            removed: removed.saturating_add(baseline.unmatched),
        }
    }
}
