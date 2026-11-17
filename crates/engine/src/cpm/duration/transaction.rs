//! The sparse journal, the work counters and the limits of one duration-cone edit.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use super::super::TaskDates;
use crate::model::ScheduleError;

/// Most distinct tasks the cone may admit, as a share of the live tasks (1 in 10)...
const MAX_ADMITTED: usize = 2_048;
/// ...and most dependency inspections per edit. Starting policies, not measured
/// crossovers (gs#58 PR 6 measures them).
pub const MAX_INSPECTIONS: usize = 16_384;

/// The two numbers at which the cone gives up: most distinct tasks it may
/// admit, and most dependency inspections. `Transaction::new` reads them once
/// per call; the `test-hooks` seam `LIMITS` replaces them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Most distinct tasks admitted by either pass.
    pub admitted: usize,
    /// Most dependency inspections, both passes.
    pub inspections: usize,
}

impl Limits {
    /// The production limits for a schedule of `live` live tasks.
    fn production(live: usize) -> Limits {
        Limits {
            admitted: live.div_ceil(10).clamp(1, MAX_ADMITTED),
            inspections: MAX_INSPECTIONS,
        }
    }
}

/// Why the cone gave up and the edit is solved by a full compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FallbackReason {
    /// The forward pass would admit more tasks than the limit.
    ForwardLimit,
    /// The backward pass would admit more tasks than the limit.
    BackwardLimit,
    /// The passes would inspect more dependencies than the limit.
    EdgeLimit,
    /// The project finish moved, so late dates move on unrelated branches.
    FinishMove,
    /// A task's early start is past its late start: let the full solve say why.
    EsAfterLs,
    /// A task is as late as possible: its place reads its successors' early dates.
    Alap,
}

/// How a cone attempt ended without a result.
#[derive(Debug)]
pub(super) enum Stop {
    Fallback(FallbackReason),
    Error(ScheduleError),
}

impl From<FallbackReason> for Stop {
    fn from(r: FallbackReason) -> Stop {
        Stop::Fallback(r)
    }
}

impl From<ScheduleError> for Stop {
    fn from(e: ScheduleError) -> Stop {
        Stop::Error(e)
    }
}

/// A multiplicative hash for `u32` task indices: the keys are dense and not
/// attacker chosen, so `SipHash` would only cost time in the hot loop.
#[derive(Default)]
pub(super) struct Mix(u64);

impl Hasher for Mix {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 32)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_u32(&mut self, i: u32) {
        self.0 = u64::from(i).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

const NOT_JOURNALED: u32 = u32::MAX;

/// What one edit has touched: the tasks admitted to either pass (each once),
/// and for those written, their original dates.
pub(super) struct Transaction {
    limits: Limits,
    inspections: usize,
    /// Admitted task -> its index in `journal`, or `NOT_JOURNALED`.
    seen: HashMap<u32, u32, BuildHasherDefault<Mix>>,
    journal: Vec<(u32, TaskDates)>,
}

impl Transaction {
    /// `live` is the number of live tasks (`Graph::order.len()`).
    pub(super) fn new(live: usize) -> Transaction {
        let limits = Limits::production(live);
        #[cfg(any(test, feature = "test-hooks"))]
        let limits = super::seam::LIMITS.get().unwrap_or(limits);
        Transaction {
            limits,
            inspections: 0,
            seen: HashMap::default(),
            journal: Vec::new(),
        }
    }

    /// Admit `si`; a task already admitted by either pass is not counted again.
    /// Fails with `reason` before admitting task `limit + 1`.
    pub(super) fn admit(&mut self, si: u32, reason: FallbackReason) -> Result<(), FallbackReason> {
        if !self.seen.contains_key(&si) {
            if self.seen.len() >= self.limits.admitted {
                return Err(reason);
            }
            self.seen.insert(si, NOT_JOURNALED);
        }
        Ok(())
    }

    /// How many distinct tasks have been admitted.
    #[cfg(any(test, feature = "test-hooks"))]
    pub(super) fn admitted(&self) -> usize {
        self.seen.len()
    }

    /// Charge `degree` dependency inspections before they are made.
    pub(super) fn charge(&mut self, degree: usize) -> Result<(), FallbackReason> {
        self.inspections += degree;
        if self.inspections > self.limits.inspections {
            return Err(FallbackReason::EdgeLimit);
        }
        Ok(())
    }

    /// Change an admitted task's dates, journaling its original ones the first time.
    pub(super) fn write(
        &mut self,
        dates: &mut [TaskDates],
        si: u32,
        change: impl FnOnce(&mut TaskDates),
    ) {
        let slot = self.seen.get_mut(&si).expect("a written task was admitted");
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the journal holds one entry per written task, so its length is below the \
                      task count (unchecked at Schedule::add_task)"
        )]
        if *slot == NOT_JOURNALED {
            *slot = self.journal.len() as u32;
            self.journal.push((si, dates[si as usize]));
        }
        change(&mut dates[si as usize]);
    }

    /// Every task written, with its dates as they were before the edit.
    pub(super) fn journal(&self) -> &[(u32, TaskDates)] {
        &self.journal
    }

    /// Put every journaled task back as it was.
    pub(super) fn restore(&self, dates: &mut [TaskDates]) {
        for &(si, old) in &self.journal {
            dates[si as usize] = old;
        }
    }
}
