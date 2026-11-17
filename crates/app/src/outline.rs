//! The task hierarchy. A task with children is a summary. This module holds
//! the forest and refuses a dependency that touches a summary task. It does
//! not yet hold collapse flags or row indexes; it computes summary rollups.

mod demo;
mod rollup;

pub use demo::{DemoLayout, demo_layout, stress_grouped_layout};
pub use rollup::{FULL_ROLLUP_DIVISOR, RollupScratch};

use crate::meta::TaskMeta;
use engine::{DepId, Schedule, TaskId};

/// An invalid task outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineError {
    /// The parent links contain a cycle (a task that is its own ancestor, a self-parent included).
    Cycle,
    /// `meta`, `wbs` and the schedule do not describe the same set of tasks: their lengths differ,
    /// `wbs` is not a permutation of the live task ids, or a live task's parent is out of range
    /// or deleted.
    WrongLength,
    /// A dependency touches a summary task (gs#47 ruling 5).
    SummaryHasDependency(DepId),
}

impl std::fmt::Display for OutlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutlineError::Cycle => write!(f, "the outline parent links contain a cycle"),
            OutlineError::WrongLength => {
                write!(f, "the outline and schedule describe different tasks")
            }
            OutlineError::SummaryHasDependency(dep) => {
                write!(f, "{dep} touches a summary task")
            }
        }
    }
}

impl std::error::Error for OutlineError {}

/// A validated task forest in work breakdown order.
///
/// Methods that take a [`TaskId`] panic when the id is out of range, and the rollups panic when
/// `dates` or `out` does not have the outline's length (the schedule's slot count, deleted tasks
/// included). Neither can happen: the ids come from the outline or the schedule or are checked at
/// the `Session` boundary, and a `Session` keeps its outline, dates and summaries at one length: a
/// delete leaves a slot (a tombstone) and an insert appends one, and both rebuild all three
/// together. `tests/outline_contracts.rs` proves each premise. A silent clamp would hide a
/// corrupted schedule.
pub struct Outline {
    parent: Vec<Option<TaskId>>,
    start: Vec<u32>,
    children: Vec<TaskId>,
    preorder: Vec<TaskId>,
    preorder_position: Vec<u32>,
    subtree_len: Vec<u32>,
}

impl Outline {
    /// Builds and validates an outline over the live tasks.
    ///
    /// `wbs` lists every live (not deleted) task id exactly once, in outline (work breakdown)
    /// order: the children of one parent keep the order they have in `wbs`, and so do the roots.
    /// A deleted task has no place in the outline: its `meta.parent` is ignored, and it is a leaf
    /// with no parent. The arrays are sized by slot count (`meta.len()`), so the rollups' length
    /// asserts keep holding. A live task whose parent is deleted is refused.
    ///
    /// # Errors
    /// An [`OutlineError`]: `Cycle` for a parent cycle, `WrongLength` when `meta`, `wbs` and the
    /// schedule do not describe the same tasks, `SummaryHasDependency` for a link on a summary.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "index and preorder.len() are task slot numbers, at most the slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots they wrap, as that id does"
    )]
    pub fn new(
        meta: &[TaskMeta],
        wbs: &[TaskId],
        schedule: &Schedule,
    ) -> Result<Outline, OutlineError> {
        let n = meta.len();
        if schedule.tasks().len() != n || wbs.len() != schedule.live_task_count() {
            return Err(OutlineError::WrongLength);
        }
        let dead = |index: usize| schedule.is_deleted(TaskId(index as u32));

        let mut seen = vec![false; n];
        for &task in wbs {
            let index = task.0 as usize;
            if index >= n || seen[index] || dead(index) {
                return Err(OutlineError::WrongLength);
            }
            seen[index] = true;
        }
        if meta.iter().enumerate().any(|(index, task)| {
            !dead(index)
                && task
                    .parent
                    .is_some_and(|parent| parent.0 as usize >= n || dead(parent.0 as usize))
        }) {
            return Err(OutlineError::WrongLength);
        }

        let parent: Vec<Option<TaskId>> = meta
            .iter()
            .enumerate()
            .map(|(index, task)| task.parent.filter(|_| !dead(index)))
            .collect();
        let mut child_count = vec![0_u32; n + 1];
        for &task in wbs {
            let parent_index = parent[task.0 as usize].map_or(n, |parent| parent.0 as usize);
            child_count[parent_index] += 1;
        }

        let mut start = vec![0_u32; n + 2];
        for node in 0..=n {
            start[node + 1] = start[node] + child_count[node];
        }

        let mut children = vec![TaskId(0); wbs.len()];
        let mut next = start[..=n].to_vec();
        for &task in wbs {
            let parent_index = parent[task.0 as usize].map_or(n, |parent| parent.0 as usize);
            children[next[parent_index] as usize] = task;
            next[parent_index] += 1;
        }

        let mut preorder = Vec::with_capacity(wbs.len());
        let mut preorder_position = vec![0_u32; n];
        let mut subtree_len = vec![0_u32; n];
        let mut stack = vec![(n, false)];
        while let Some((node, exiting)) = stack.pop() {
            if exiting {
                subtree_len[node] = preorder.len() as u32 - preorder_position[node];
                continue;
            }

            if node < n {
                preorder_position[node] = preorder.len() as u32;
                preorder.push(TaskId(node as u32));
                stack.push((node, true));
            }

            let child_range = start[node] as usize..start[node + 1] as usize;
            for child in children[child_range].iter().rev() {
                stack.push((child.0 as usize, false));
            }
        }
        if preorder.len() != wbs.len() {
            return Err(OutlineError::Cycle);
        }

        // Only a link in force counts: a removed one, or one with a deleted end, may touch a summary.
        for (dep, dependency) in schedule.live_dependencies() {
            let pred = dependency.pred.0 as usize;
            let succ = dependency.succ.0 as usize;
            if start[pred] != start[pred + 1] || start[succ] != start[succ + 1] {
                return Err(OutlineError::SummaryHasDependency(dep));
            }
        }

        Ok(Outline {
            parent,
            start,
            children,
            preorder,
            preorder_position,
            subtree_len,
        })
    }

    /// Returns the slot count: every task id, deleted ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Returns whether the outline has no tasks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Returns the task's parent, or `None` when it is a root.
    #[must_use]
    pub fn parent(&self, task: TaskId) -> Option<TaskId> {
        self.parent[task.0 as usize]
    }

    /// Returns the task's one-based depth, where a root has depth 1.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a depth counts one task's ancestors, so it is at most the slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps"
    )]
    #[must_use]
    pub fn depth(&self, task: TaskId) -> u32 {
        std::iter::successors(Some(task), |task| self.parent(*task)).count() as u32
    }

    /// Returns the task's children in `wbs` order.
    ///
    /// # Panics
    ///
    /// Panics if `task` is out of range. Unreachable today (`tests/outline_contracts.rs`).
    #[must_use]
    pub fn children(&self, task: TaskId) -> &[TaskId] {
        let task = task.0 as usize;
        assert!(task < self.len(), "task id out of range");
        &self.children[self.start[task] as usize..self.start[task + 1] as usize]
    }

    /// Returns the forest's roots in `wbs` order.
    #[must_use]
    pub fn roots(&self) -> &[TaskId] {
        let root = self.len();
        &self.children[self.start[root] as usize..self.start[root + 1] as usize]
    }

    /// Returns whether the task has at least one child.
    ///
    /// # Panics
    ///
    /// Panics if `task` is out of range. Unreachable today (`tests/outline_contracts.rs`).
    #[must_use]
    pub fn is_summary(&self, task: TaskId) -> bool {
        let task = task.0 as usize;
        assert!(task < self.len(), "task id out of range");
        self.start[task] != self.start[task + 1]
    }

    /// Returns all tasks with parents before children and siblings in `wbs` order.
    #[must_use]
    pub fn preorder(&self) -> &[TaskId] {
        &self.preorder
    }

    /// Returns the task's position in [`Self::preorder`].
    pub(crate) fn preorder_position(&self, task: TaskId) -> usize {
        self.preorder_position[task.0 as usize] as usize
    }

    /// Returns the task followed by all its descendants in preorder.
    #[must_use]
    pub fn subtree(&self, task: TaskId) -> &[TaskId] {
        let task = task.0 as usize;
        let position = self.preorder_position[task] as usize;
        let end = position + self.subtree_len[task] as usize;
        &self.preorder[position..end]
    }
}
