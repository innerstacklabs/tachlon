//! The ordered work queue of one cone pass (gs#58 PR 3).
//!
//! A pass must visit tasks in topological order: forward by ascending rank
//! (a task after all its predecessors), backward by descending rank. The queue
//! holds only tasks the pass has discovered, each once, and hands them out in
//! that order. Nothing in it is sized by the number of tasks in the schedule:
//! the ranks come from the cached `Graph::rank`, and the queued set grows
//! only with the work found.
//!
//! A pass pushes a task only from a task of lower rank (forward) or higher
//! rank (backward), so a task that was handed out is never pushed again.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::hash::BuildHasherDefault;

use super::transaction::Mix;
use crate::model::Graph;

/// Which way a pass walks the topological order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    /// Successors, by ascending rank.
    Forward,
    /// Predecessors, by descending rank.
    Backward,
}

/// Discovered tasks of one pass, each once, in the pass's order.
pub(super) struct WorkQueue<'g> {
    g: &'g Graph,
    direction: Direction,
    /// Min-heap of keys: the rank forward, its mirror backward.
    heap: BinaryHeap<Reverse<u32>>,
    /// The keys on `heap`, so a task reached from several sides is queued once.
    queued: HashSet<u32, BuildHasherDefault<Mix>>,
}

impl<'g> WorkQueue<'g> {
    pub(super) fn new(g: &'g Graph, direction: Direction) -> WorkQueue<'g> {
        WorkQueue {
            g,
            direction,
            heap: BinaryHeap::new(),
            queued: HashSet::default(),
        }
    }

    /// The heap key of a rank: ascending in the pass's order.
    fn key(&self, rank: u32) -> u32 {
        match self.direction {
            Direction::Forward => rank,
            Direction::Backward => u32::MAX - rank,
        }
    }

    /// Queue a live task. Returns false, and changes nothing, when it is
    /// already queued.
    pub(super) fn push(&mut self, task: u32) -> bool {
        let key = self.key(self.g.rank[task as usize]);
        let fresh = self.queued.insert(key);
        if fresh {
            self.heap.push(Reverse(key));
            #[cfg(any(test, feature = "test-hooks"))]
            super::seam::enqueued();
        }
        fresh
    }

    /// The next task in the pass's order.
    pub(super) fn pop(&mut self) -> Option<u32> {
        let Reverse(key) = self.heap.pop()?;
        self.queued.remove(&key);
        // The mirror is its own inverse, so `key` also decodes a backward key.
        Some(self.g.order[self.key(key) as usize])
    }
}

#[cfg(test)]
mod tests;
