//! The DCMA link checks as counts: missing logic with its open starts and open ends, leads, lags
//! and the share of finish-to-start links. Summaries are skipped for those three, which is why this
//! lives here and not in `engine`: only the session's outline knows which tasks are summaries.

pub mod baseline;
pub mod report;
pub mod schedule;

use crate::{Session, check_filter::LinkEnds};
use engine::{LinkType, TaskId};

/// Counts over the live tasks and links of one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checks {
    /// Unfinished leaves with no predecessor or no successor.
    pub missing_logic: usize,
    /// Unfinished leaves with no predecessor.
    pub open_starts: usize,
    /// Unfinished leaves with no successor.
    pub open_ends: usize,
    /// Links with a negative lag.
    pub leads: usize,
    /// Links with a positive lag.
    pub lags: usize,
    /// Live links.
    pub links: usize,
    /// Live finish-to-start links.
    pub finish_to_start: usize,
}

impl Checks {
    /// Counts the checks for `session`. Deleted tasks and links with a deleted end are left out;
    /// without an outline every task is a leaf.
    pub fn of(session: &Session) -> Checks {
        let schedule = session.schedule();
        let tasks = schedule.tasks();
        let ends = LinkEnds::of(schedule);
        let mut checks = Checks {
            missing_logic: 0,
            open_starts: 0,
            open_ends: 0,
            leads: 0,
            lags: 0,
            links: 0,
            finish_to_start: 0,
        };

        for (_, dep) in schedule.live_dependencies() {
            checks.links += 1;
            checks.leads += usize::from(dep.lag < 0);
            checks.lags += usize::from(dep.lag > 0);
            checks.finish_to_start += usize::from(dep.kind == LinkType::FS);
        }

        for (index, task) in tasks.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let id = TaskId(index as u32);
            let summary = session
                .outline()
                .is_some_and(|outline| outline.is_summary(id));
            if task.deleted || summary || task.actual_finish.is_some() {
                continue;
            }
            checks.missing_logic += usize::from(!ends.has_pred[index] || !ends.has_succ[index]);
            checks.open_starts += usize::from(!ends.has_pred[index]);
            checks.open_ends += usize::from(!ends.has_succ[index]);
        }
        checks
    }

    /// The share of links that are finish-to-start, from 0 to 1; `None` when there are no links.
    #[expect(
        clippy::cast_precision_loss,
        reason = "link counts, far under 2^53 (that many links would not fit in memory), so f64 holds both exactly"
    )]
    #[must_use]
    pub fn finish_to_start_share(&self) -> Option<f64> {
        (self.links > 0).then(|| self.finish_to_start as f64 / self.links as f64)
    }
}
