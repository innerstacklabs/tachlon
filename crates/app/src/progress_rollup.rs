//! The % complete of a summary row (gs#703): the share of the work under it that is done,
//! weighted by duration in working minutes. The sums for every task come from one pass over the
//! outline and are kept for the `(revision, stamp)` they were made at, as the baseline summary
//! keeps its own: a drag moves the stamp at every step, but the table asks for its visible rows
//! only when it moves, and the pass costs one read of the leaves, not one per row.

use crate::Session;
use engine::TaskId;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

/// What lies under one task: the working minutes done, the working minutes in all, and whether
/// any leaf has begun.
#[derive(Debug, Default, Clone, Copy)]
struct Sum {
    done: i128,
    total: i128,
    begun: bool,
}

/// The sums of every task, with the `(revision, stamp)` they were made at.
type Kept = ((u32, u32), Vec<Sum>);

/// The sums kept by a [`Session`] and the count of passes that made them. A session made by
/// `new` or `reopen` starts with none, so a file opened over another never reads its sums.
#[derive(Debug, Default)]
pub(crate) struct Memo {
    cached: Mutex<Option<Kept>>,
    runs: AtomicU32,
}

impl Session {
    /// The percent complete of summary `task` as the leaf column writes it, or an empty string
    /// while no leaf under it has begun or all of them are milestones (no duration to weigh).
    pub(crate) fn summary_percent(&self, task: TaskId) -> String {
        let key = (self.revision(), self.stamp());
        let mut cached = self
            .progress_rollup
            .cached
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if cached.as_ref().is_none_or(|(kept, _)| *kept != key) {
            self.progress_rollup.runs.fetch_add(1, Ordering::Relaxed);
            *cached = Some((key, self.sum_leaves()));
        }
        let sum = cached.as_ref().map(|(_, sums)| sums[task.0 as usize]);
        match sum {
            Some(Sum {
                done,
                total,
                begun: true,
            }) if total > 0 => format!("{}%", (done * 200 + total) / (2 * total)),
            _ => String::new(),
        }
    }

    /// How many times the sums have been made.
    pub fn progress_rollup_runs(&self) -> u32 {
        self.progress_rollup.runs.load(Ordering::Relaxed)
    }

    /// The sums of every task, children before parents. A summary's own task holds no work of its
    /// own: only the leaves under it count.
    fn sum_leaves(&self) -> Vec<Sum> {
        let mut sums = vec![Sum::default(); self.schedule().tasks().len()];
        let Some(outline) = self.outline() else {
            return sums;
        };
        for &task in outline.preorder().iter().rev() {
            let sum = if outline.is_summary(task) {
                outline
                    .children(task)
                    .iter()
                    .fold(Sum::default(), |acc, child| {
                        let c = sums[child.0 as usize];
                        Sum {
                            done: acc.done + c.done,
                            total: acc.total + c.total,
                            begun: acc.begun || c.begun,
                        }
                    })
            } else {
                let leaf = &self.schedule().tasks()[task.0 as usize];
                match Self::remaining_minutes(leaf) {
                    Some(remaining) => Sum {
                        done: i128::from(leaf.duration - remaining),
                        total: i128::from(leaf.duration),
                        begun: true,
                    },
                    None => Sum {
                        done: 0,
                        total: i128::from(leaf.duration),
                        begun: false,
                    },
                }
            };
            sums[task.0 as usize] = sum;
        }
        sums
    }
}
