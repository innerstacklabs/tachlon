//! The commits a host has not acknowledged (decision 0019, rule 11): what `rollbackTo` walks back.

use crate::history::{HISTORY_CAP, Source};
use std::collections::VecDeque;

/// What a commit did to the two history stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingKind {
    /// Pushed an undo entry and cleared redo.
    Pushed,
    /// Moved the latest undo entry to redo.
    Undid,
    /// Moved the latest redo entry to undo.
    Redid,
}

#[derive(Default)]
pub(crate) struct Pending(VecDeque<(u64, PendingKind)>);

impl Pending {
    /// Keep one step of the commit `seq` made by `source`; a commit that took several steps is kept once for
    /// each. One past [`HISTORY_CAP`] drops every step of the oldest commit, and a rollback to it then refuses.
    pub(crate) fn push(&mut self, seq: u64, source: Source) {
        let kind = match source {
            Source::Undo => PendingKind::Undid,
            Source::Redo => PendingKind::Redid,
            _ => PendingKind::Pushed,
        };
        if self.0.len() >= HISTORY_CAP {
            let oldest = self.0.front().map(|&(kept, _)| kept);
            self.0.retain(|&(kept, _)| Some(kept) != oldest);
        }
        self.0.push_back((seq, kind));
    }

    /// The kinds of the commits from `seq` to the latest, newest first; none when `seq` is not kept.
    pub(crate) fn walk(&self, seq: u64) -> Option<Vec<PendingKind>> {
        let at = self.0.iter().position(|&(kept, _)| kept == seq)?;
        Some(
            self.0
                .iter()
                .skip(at)
                .rev()
                .map(|&(_, kind)| kind)
                .collect(),
        )
    }

    /// Forget the commits up to and including `seq`: the host saved them.
    pub(crate) fn acknowledge(&mut self, seq: u64) {
        self.0.retain(|&(kept, _)| kept > seq);
    }

    /// Forget the commits from `seq` on: a rollback took them back.
    pub(crate) fn truncate(&mut self, seq: u64) {
        self.0.retain(|&(kept, _)| kept < seq);
    }
}
