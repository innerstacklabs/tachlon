//! Summary rollups: the full pass, the moved-only pass, and the hybrid that picks between them.

use super::Outline;
use crate::Session;
use engine::{Moved, TaskDates, TaskId};
use timeline::Summaries;

/// Reusable scratch for [`Outline::rollup_moved`], so a drag allocates nothing.
#[derive(Debug, Default)]
pub struct RollupScratch {
    marks: Vec<u32>,
    epoch: u32,
    dirty: Vec<TaskId>,
    /// The ids of a [`Moved`] list, for [`Outline::rollup_from_moved`].
    ids: Vec<TaskId>,
}

/// `moved.len() * FULL_ROLLUP_DIVISOR > task_count` runs the full pass. The
/// plan started at 8; the sweep in `tests/summary.rs` (`hybrid_threshold_sweep`)
/// shows the incremental path still ahead up to n/2 at 10k, 50k and 100k tasks.
pub const FULL_ROLLUP_DIVISOR: usize = 2;

impl Outline {
    /// Rebuilds every summary span. `dates` and `out` must match the outline length.
    ///
    /// # Panics
    ///
    /// Panics if either length differs. Unreachable today (`tests/outline_contracts.rs`).
    pub fn rollup_full(&self, dates: &[TaskDates], out: &mut Summaries) {
        assert_eq!(dates.len(), self.len(), "dates length must match outline");
        assert_eq!(out.len(), self.len(), "summary length must match outline");

        for &task in self.preorder.iter().rev() {
            let span = self
                .is_summary(task)
                .then(|| self.summary_span(task, dates, out));
            out.set(task.0, span);
        }
    }

    /// Rebuilds summaries above tasks whose early dates moved.
    ///
    /// Its cost grows with the fan-out of the dirty ancestors. `dates` and
    /// `out` must match the outline length.
    ///
    /// # Panics
    ///
    /// Panics if either length differs. Unreachable today (`tests/outline_contracts.rs`).
    pub fn rollup_moved(
        &self,
        dates: &[TaskDates],
        moved: &[TaskId],
        scratch: &mut RollupScratch,
        out: &mut Summaries,
    ) {
        assert_eq!(dates.len(), self.len(), "dates length must match outline");
        assert_eq!(out.len(), self.len(), "summary length must match outline");

        if scratch.marks.len() < self.len() {
            scratch.marks.resize(self.len(), 0);
        }
        scratch.epoch = scratch.epoch.wrapping_add(1);
        if scratch.epoch == 0 {
            scratch.marks.fill(0);
            scratch.epoch = 1;
        }
        scratch.dirty.clear();

        for &moved_task in moved {
            let mut ancestor = self.parent(moved_task);
            while let Some(task) = ancestor {
                let index = task.0 as usize;
                if scratch.marks[index] == scratch.epoch {
                    break;
                }
                scratch.marks[index] = scratch.epoch;
                scratch.dirty.push(task);
                ancestor = self.parent(task);
            }
        }

        scratch.dirty.sort_unstable_by(|a, b| {
            self.preorder_position[b.0 as usize].cmp(&self.preorder_position[a.0 as usize])
        });
        for &task in &scratch.dirty {
            out.set(task.0, Some(self.summary_span(task, dates, out)));
        }
    }

    /// Rebuilds summary spans with a full or moved-only pass, according to
    /// the size of the moved set.
    pub fn rollup(
        &self,
        dates: &[TaskDates],
        moved: &[TaskId],
        scratch: &mut RollupScratch,
        out: &mut Summaries,
    ) {
        if moved.len() * FULL_ROLLUP_DIVISOR > self.len() {
            self.rollup_full(dates, out);
        } else {
            self.rollup_moved(dates, moved, scratch, out);
        }
    }

    /// [`Outline::rollup`] for the list a cone re-solve reports: it takes the
    /// task ids out of `moved`, in `scratch`, so a drag allocates nothing.
    pub fn rollup_from_moved(
        &self,
        dates: &[TaskDates],
        moved: &[Moved],
        scratch: &mut RollupScratch,
        out: &mut Summaries,
    ) {
        let mut ids = std::mem::take(&mut scratch.ids);
        ids.clear();
        ids.extend(moved.iter().map(|m| m.task));
        self.rollup(dates, &ids, scratch, out);
        scratch.ids = ids;
    }

    fn summary_span(
        &self,
        task: TaskId,
        dates: &[TaskDates],
        summaries: &Summaries,
    ) -> (engine::Instant, engine::Instant) {
        self.children(task)
            .iter()
            .map(|&child| {
                if self.is_summary(child) {
                    summaries
                        .get(child.0)
                        .expect("child summary must be rolled up first")
                } else {
                    let dates = &dates[child.0 as usize];
                    (dates.es, dates.ef)
                }
            })
            .reduce(|(min_es, max_ef), (es, ef)| (min_es.min(es), max_ef.max(ef)))
            .expect("a summary has at least one child")
    }
}

impl Session {
    pub(crate) fn rollup_full_summaries(&mut self) {
        let Self {
            solution,
            timeline,
            outline,
            ..
        } = self;
        if let Some(outline) = outline.as_ref() {
            outline.rollup_full(&solution.dates, timeline.summaries_mut());
        }
    }

    pub(crate) fn rollup_moved_summaries(&mut self) {
        let Self {
            solution,
            timeline,
            outline,
            moved,
            rollup_scratch,
            ..
        } = self;
        if let Some(outline) = outline.as_ref() {
            outline.rollup_from_moved(
                &solution.dates,
                moved,
                rollup_scratch,
                timeline.summaries_mut(),
            );
        }
    }
}
