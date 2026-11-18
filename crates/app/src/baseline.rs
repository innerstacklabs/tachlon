//! The baseline (gs#703): a snapshot of the schedule's dates that a person sets and the session
//! keeps, so a later change can be read against it. Setting and clearing it is undoable, and
//! [`Session::baseline_diff`] reads the schedule against it for the status line.
//!
//! A [`Baseline`] is immutable and shared as an `Arc`, so an [`Edit`] can carry the one that was in
//! force and undo and redo hand back the same snapshot, not a recomputed one. It belongs to one
//! schedule: [`Session::reopen`] starts without one.

mod summary;
use summary::{BaselineJson, Memo};
pub use summary::{BaselineSummary, DRAG_SUMMARY_INTERVAL_MS, SUMMARY_JSON_MAX};

use crate::edit::{Edit, EditError};
use crate::{RowFilter, Session};
use engine::{Instant, Minutes, Schedule, ScheduleError, Solution, TaskDates, TaskId};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// What the baseline remembers about one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineTask {
    /// Early start.
    pub es: Instant,
    /// Early finish.
    pub ef: Instant,
    /// Total float, in working minutes of the task's calendar. `None` for a baseline a file saved:
    /// MS Project states dates, not float, so such a task joins or leaves the critical path in no
    /// count.
    pub float: Option<Minutes>,
}

/// A snapshot of every live task's early start, early finish and float, and the project finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    /// Indexed by [`TaskId`]; `None` for a task that was deleted when the snapshot was taken.
    tasks: Vec<Option<BaselineTask>>,
    project_finish: Instant,
    /// The session's data date when the snapshot was taken, `None` when it had none or the baseline
    /// came from a file that does not say. It is what tells one set from the next.
    data_date: Option<Instant>,
    /// Tasks of the older file this baseline was read from that are not in the session: it has no
    /// slot for them, so the summary counts them as removed. Zero for any other baseline.
    unmatched: u32,
    /// The last [`BaselineSummary`] read against this snapshot. Not part of what the baseline is:
    /// a clone starts without it and two baselines are equal whatever they remember.
    memo: Memo,
}

impl Baseline {
    /// The snapshot of `solution`, the solution of `schedule`.
    #[must_use]
    pub fn snapshot(
        schedule: &Schedule,
        solution: &Solution,
        data_date: Option<Instant>,
    ) -> Baseline {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
        )]
        let tasks = solution
            .dates
            .iter()
            .enumerate()
            .map(|(i, dates)| {
                (!schedule.is_deleted(TaskId(i as u32))).then_some(BaselineTask {
                    es: dates.es,
                    ef: dates.ef,
                    float: Some(dates.float),
                })
            })
            .collect();
        Baseline {
            tasks,
            project_finish: solution.project_finish,
            data_date,
            unmatched: 0,
            memo: Memo::default(),
        }
    }

    /// A baseline from its parts, as a saved document holds them: `tasks` indexed by [`TaskId`],
    /// `None` for a task that was deleted, and the project finish.
    #[must_use]
    pub fn from_parts(tasks: Vec<Option<BaselineTask>>, project_finish: Instant) -> Baseline {
        Baseline {
            tasks,
            project_finish,
            data_date: None,
            unmatched: 0,
            memo: Memo::default(),
        }
    }

    /// This baseline with `unmatched` tasks of its file that the session does not have.
    #[must_use]
    pub fn with_unmatched(mut self, unmatched: u32) -> Baseline {
        self.unmatched = unmatched;
        self
    }

    /// This baseline with an entry, at their current dates and float, for every live task of
    /// `schedule` it has none for. Every other entry, and the project finish, stay as they were, so
    /// the slips already recorded do not move.
    #[must_use]
    pub fn with_new_tasks(&self, schedule: &Schedule, solution: &Solution) -> Baseline {
        let mut tasks = self.tasks.clone();
        tasks.resize(tasks.len().max(solution.dates.len()), None);
        for (i, dates) in solution.dates.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            if tasks[i].is_none() && !schedule.is_deleted(TaskId(i as u32)) {
                tasks[i] = Some(BaselineTask {
                    es: dates.es,
                    ef: dates.ef,
                    float: Some(dates.float),
                });
            }
        }
        Baseline::from_parts(tasks, self.project_finish)
            .with_data_date(self.data_date)
            .with_unmatched(self.unmatched)
    }

    /// This baseline with `task`'s entry reset to its current dates and float; every other entry,
    /// the project finish and the data date stay. `None` when it has no entry for `task`.
    #[must_use]
    pub fn with_task_reset(&self, task: TaskId, solution: &Solution) -> Option<Baseline> {
        self.task(task)?;
        let dates = solution.dates.get(task.0 as usize)?;
        let mut reset = self.clone();
        reset.tasks[task.0 as usize] = Some(BaselineTask {
            es: dates.es,
            ef: dates.ef,
            float: Some(dates.float),
        });
        Some(reset)
    }

    /// This baseline as set at `data_date`.
    #[must_use]
    pub fn with_data_date(mut self, data_date: Option<Instant>) -> Baseline {
        self.data_date = data_date;
        self
    }

    /// The data date the session had when the baseline was set.
    pub fn data_date(&self) -> Option<Instant> {
        self.data_date
    }

    /// How many times [`Session::baseline_diff`] has computed its summary against this snapshot
    /// (a read of the one it kept does not count).
    pub fn diffs_computed(&self) -> u32 {
        self.memo.runs.load(Ordering::Relaxed)
    }

    /// The snapshot as a [`Solution`] for [`engine::diff`], which reads only each slot's early
    /// dates and float. A slot with no entry is all zero, as a deleted task's is in a solution; the
    /// summary reads liveness from the baseline and the schedule, not from this.
    pub(crate) fn as_solution(&self) -> &Solution {
        self.memo.solution.get_or_init(|| self.build_solution())
    }

    fn build_solution(&self) -> Solution {
        let dates = self
            .tasks
            .iter()
            .map(|slot| {
                slot.map_or(
                    TaskDates {
                        es: 0,
                        ef: 0,
                        ls: 0,
                        lf: 0,
                        float: 0,
                        finished: false,
                    },
                    |t| TaskDates {
                        es: t.es,
                        ef: t.ef,
                        ls: t.es,
                        lf: t.ef,
                        float: t.float.unwrap_or(0),
                        // The snapshot keeps float, not progress: a task critical in it is one
                        // that had zero float then.
                        finished: false,
                    },
                )
            })
            .collect();
        Solution {
            dates,
            project_finish: self.project_finish,
        }
    }

    /// What the baseline holds for `task`. `None` for a task that was deleted, or that did not
    /// exist (inserted since), when the baseline was set.
    pub fn task(&self, task: TaskId) -> Option<&BaselineTask> {
        self.tasks.get(task.0 as usize)?.as_ref()
    }

    /// The number of tasks the baseline holds.
    pub fn len(&self) -> usize {
        self.tasks.iter().flatten().count()
    }

    /// Whether the baseline holds no task.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The project finish when the baseline was set.
    pub fn project_finish(&self) -> Instant {
        self.project_finish
    }
}

impl Session {
    /// The baseline in force, if one is set.
    pub fn baseline(&self) -> Option<&Arc<Baseline>> {
        self.baseline.as_ref()
    }

    /// Put `baseline` in force as the session opens, as a document that holds one is read. It is
    /// not an edit: it leaves no undo step, so the history stays empty.
    pub fn with_baseline(mut self, baseline: Baseline) -> Session {
        self.baseline = Some(Arc::new(baseline));
        self.baseline_changed();
        self
    }

    /// The day the baseline was set at, as the toolbar says it (`14 Oct`), or `None` with no baseline
    /// or one set with no data date.
    pub fn baseline_data_date_text(&self) -> Option<String> {
        let at = self.baseline.as_ref()?.data_date()?;
        Some(self.day_month(at))
    }

    /// The day the baseline was set at, written `YYYY-MM-DD` (the form the data date is set in), or
    /// `None` with no baseline or one set with no data date.
    pub fn baseline_data_day(&self) -> Option<String> {
        Some(self.origin.day_text(self.baseline.as_ref()?.data_date()?))
    }

    /// How the schedule differs from the baseline, or `None` with no baseline set.
    ///
    /// `status()` reads this every frame, so it is computed once per solution and kept: the key is
    /// the revision and the stamp, because a drag restore puts the revision back while the stamp
    /// still goes up (the same revision can then hold a different solution). With no baseline
    /// nothing is computed, and a frame that finds the key unchanged reads the kept summary.
    ///
    /// The summary reads every task, and a drag makes a new solution each frame, so during a drag
    /// it is computed at most once per [`DRAG_SUMMARY_INTERVAL_MS`] and the frames between read the
    /// last value. It cannot follow the edit's moved list: that holds the tasks whose early dates
    /// changed, not those whose float did, and a project finish that moves redoes every float. At
    /// the release the key has moved on and the next read computes the exact summary.
    pub fn baseline_diff(&self) -> Option<BaselineSummary> {
        let baseline = self.baseline.as_ref()?;
        let key = (self.revision(), self.stamp());
        let dragging = self.drag_started();
        Some(
            baseline
                .memo
                .get(key, dragging, self.clock, || self.summarize(baseline)),
        )
    }

    /// The `baseline` value of `status_json`: `null` with no baseline, else the summary's counts.
    pub(crate) fn baseline_json(&self) -> BaselineJson {
        BaselineJson(self.baseline_diff())
    }

    /// Add the comparison bars to `scene`: under each baselined visible leaf bar, a thin bar at the
    /// baseline's dates. They read the baseline, not `display_dates`, so a slide or a drag does not
    /// move them. With no baseline it returns at once, before anything is allocated.
    pub fn add_baseline_bars(&self, scene: &mut timeline::Scene) {
        let Some(baseline) = &self.baseline else {
            return;
        };
        let tl = self.timeline();
        timeline::scene::add_comparison_bars(
            scene,
            &tl.viewport,
            tl.dpr,
            tl.summaries(),
            tl.rows(),
            |task| baseline.task(TaskId(task)).map(|t| (t.es, t.ef)),
            (0, self.theme().baseline),
        );
    }

    /// The table redraws on a stamp change, and the shell reads the baseline when it moves. The
    /// Slipped, Joined critical and Left critical filters read the baseline, so they follow it, and
    /// go off when there is none.
    fn baseline_changed(&mut self) {
        if self.baseline.is_none() && self.row_filter.needs_baseline() {
            self.row_filter = RowFilter::Off;
        }
        let before = self.stamp();
        if self.filtering() {
            self.follow_filter();
        } else {
            self.apply_rows();
        }
        // A row change has bumped the stamp already; one change bumps it once.
        if self.stamp() == before {
            self.stamp = before.wrapping_add(1);
        }
    }

    /// Set a baseline: `with` when it is `Some`, else a snapshot of the solution as it is. The
    /// inverse puts back the one that was in force: the same `Arc`, or a clear when there was none.
    pub(crate) fn run_set_baseline(&mut self, with: Option<Arc<Baseline>>) -> (Edit, f64) {
        let new = with.unwrap_or_else(|| {
            Arc::new(Baseline::snapshot(
                &self.schedule,
                &self.solution,
                self.data_date,
            ))
        });
        let inverse = match self.baseline.replace(new) {
            Some(before) => Edit::SetBaseline {
                baseline: Some(before),
            },
            None => Edit::ClearBaseline,
        };
        self.baseline_changed();
        (inverse, 0.0)
    }

    /// Give the tasks added since the baseline their current dates as baseline, leaving every
    /// other entry as it was: one [`Edit::SetBaseline`] step, so undo and redo are the existing
    /// ones. Refused when there is no baseline.
    ///
    /// # Errors
    /// [`EditError::NoBaseline`] when there is no baseline, else the edit's refusal.
    pub fn run_add_new_to_baseline(&mut self) -> Result<(), EditError> {
        let with = self
            .baseline
            .as_ref()
            .ok_or(EditError::NoBaseline)?
            .with_new_tasks(&self.schedule, &self.solution);
        self.edit(Edit::SetBaseline {
            baseline: Some(Arc::new(with)),
        })
    }

    /// Reset `task`'s baseline entry to its current dates and float, leaving every other entry as
    /// it was: one [`Edit::SetBaseline`] step, so undo and redo are the existing ones.
    ///
    /// # Errors
    /// [`EditError::NoBaseline`], a deleted task, [`EditError::BaselineOnSummary`] or
    /// [`EditError::NotInBaseline`], else the edit's refusal.
    pub fn run_rebaseline_task(&mut self, task: TaskId) -> Result<(), EditError> {
        let baseline = self.baseline.as_ref().ok_or(EditError::NoBaseline)?;
        if !self.is_live(task) {
            return Err(ScheduleError::UnknownTask(task).into());
        }
        let name = || self.meta[task.0 as usize].name.clone();
        if self.outline.as_ref().is_some_and(|o| o.is_summary(task)) {
            return Err(EditError::BaselineOnSummary { name: name() });
        }
        let with = baseline
            .with_task_reset(task, &self.solution)
            .ok_or_else(|| EditError::NotInBaseline { name: name() })?;
        self.edit(Edit::SetBaseline {
            baseline: Some(Arc::new(with)),
        })
    }

    /// Clear the baseline. Refused when there is none, so no step records nothing.
    pub(crate) fn run_clear_baseline(&mut self) -> Result<(Edit, f64), EditError> {
        let before = self.baseline.take().ok_or(EditError::NoBaseline)?;
        self.baseline_changed();
        let inverse = Edit::SetBaseline {
            baseline: Some(before),
        };
        Ok((inverse, 0.0))
    }
}
