//! Selection: the hover card ([`TaskDetails`]), the selected task and its driving path
//! ([`Selection`]), and the walk that builds it (gs#243, split out of `lib.rs`).

use crate::{Session, edit, meta};
use engine::TaskId;
use std::collections::HashSet;

/// Most tasks the chain holds each way.
pub const CHAIN_CAP: usize = 2_000;

mod details;
pub use details::TaskDetails;

/// A selected task and its driving path.
#[derive(Debug, Clone)]
pub struct Selection {
    pub task: u32,
    /// Row of `task`, or `timeline::HIDDEN_ROW` while it is hidden by a collapsed summary.
    pub row: u32,
    /// Driving predecessors, breadth first from `task` (nearest first), at most `CHAIN_CAP`.
    pub back: Vec<u32>,
    /// More driving predecessors exist than `back` holds.
    pub back_capped: bool,
    /// Driven successors, breadth first, at most `CHAIN_CAP`.
    pub forward: Vec<u32>,
    pub forward_capped: bool,
    highlight: timeline::scene::Highlight,
}

impl Selection {
    #[must_use]
    pub fn highlight(&self) -> &timeline::scene::Highlight {
        &self.highlight
    }
}

impl Session {
    /// The display data of every task, indexed like [`engine::Schedule::tasks`].
    pub fn meta(&self) -> &[meta::TaskMeta] {
        &self.meta
    }

    /// The outline path of task `task`: the names of its ancestors and its own, outermost first,
    /// joined by `" / "`, or an empty string if absent or deleted. The path is derived from the parents, so
    /// renaming a task changes the paths below it without touching their names.
    pub fn outline_path(&self, task: u32) -> String {
        if !self.is_live(TaskId(task)) {
            return String::new();
        }
        meta::outline_path(&self.meta, task as usize)
    }

    /// The activity ID of task `i` as the ID column shows it, or an empty string when the task has
    /// none, is absent or is deleted.
    pub fn task_code(&self, i: u32) -> &str {
        if !self.is_live(TaskId(i)) {
            return "";
        }
        self.meta
            .get(i as usize)
            .map_or("", meta::TaskMeta::code_text)
    }

    /// Whether any live task has an activity ID. The table shows the ID column only then, so it
    /// hides when the last coded task is deleted and shows again on undo.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the task slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
    )]
    pub fn has_codes(&self) -> bool {
        (0..self.meta.len() as u32)
            .any(|i| self.is_live(TaskId(i)) && self.meta[i as usize].code.is_some())
    }

    /// The own name of task `i` (its last outline segment), or an empty string if absent or deleted.
    pub fn task_label(&self, i: u32) -> &str {
        if !self.is_live(TaskId(i)) {
            return "";
        }
        self.meta
            .get(i as usize)
            .map_or("", |task| task.name.as_str())
    }

    /// The outline level (1 to 3) of task `i`, or zero if absent or deleted.
    pub fn task_outline_level(&self, i: u32) -> u8 {
        if !self.is_live(TaskId(i)) {
            return 0;
        }
        self.meta
            .get(i as usize)
            .map_or(0, |task| task.outline_level)
    }

    /// The duration of task `i` as the table shows it, or an empty string if absent or deleted.
    /// A summary shows the working time of its calendar from its earliest start to its latest
    /// finish, the span its Start and Finish cells show; a leaf shows its own duration.
    pub fn task_duration(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        let Some(task) = self.schedule().tasks().get(i as usize) else {
            return String::new();
        };
        let calendar = self.schedule().calendar_of(TaskId(i));
        let minutes = match self.summary_span(i) {
            Some((start, finish)) => calendar.working_between(start, finish),
            None => task.duration,
        };
        edit::format_duration(minutes, edit::day_minutes(calendar))
    }

    /// The total float of task `i` as the table shows it, in working days of the task's
    /// calendar (`3d`, `2.5d`), or an empty string if absent or deleted.
    pub fn task_float(&self, i: u32) -> String {
        if !self.is_live(TaskId(i)) {
            return String::new();
        }
        let Some(dates) = self.solution().dates.get(i as usize) else {
            return String::new();
        };
        let day = edit::day_minutes(self.schedule().calendar_of(TaskId(i)));
        edit::format_float_days(dates.float, day)
    }

    /// Everything the hover card shows for `task`, or `None` past the schedule.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a dependency index; Schedule::add_dependency casts the count to u32 with no check against u32::MAX, so past 2^32 links it wraps, as that id does"
    )]
    pub fn task_details(&self, task: u32) -> Option<TaskDetails> {
        if !self.is_live(TaskId(task)) {
            return None;
        }
        let task_index = task as usize;

        let dates = &self.solution().dates[task_index];
        let outline_path = self.outline_path(task);
        let name = self.task_label(task).to_string();

        let calendar = self.schedule().calendar_of(TaskId(task));
        let day_len = edit::day_minutes(calendar);
        let float_days = dates.float / day_len;
        let float_hours = (dates.float % day_len) / 60;
        let float_rem_minutes = (dates.float % day_len) % 60;
        let float_text = details::float_text(dates.float, day_len);

        let mut predecessor_ids = HashSet::new();
        let mut successor_ids = HashSet::new();
        let mut predecessor_count = 0;
        let mut successor_count = 0;
        let mut predecessors = Vec::new();
        let mut successors = Vec::new();
        for (index, dependency) in self.schedule().dependencies().iter().enumerate() {
            if !self
                .schedule()
                .is_dependency_live(engine::DepId(index as u32))
            {
                continue;
            }
            if dependency.succ.0 == task && predecessor_ids.insert(dependency.pred) {
                predecessor_count += 1;
                if predecessors.len() < 3 {
                    predecessors.push(self.related_line(dependency.pred.0, index));
                }
            }
            if dependency.pred.0 == task && successor_ids.insert(dependency.succ) {
                successor_count += 1;
                if successors.len() < 3 {
                    successors.push(self.related_line(dependency.succ.0, index));
                }
            }
        }

        let actual_start = self.task_actual_start(task);
        let actual_finish = self.task_actual_finish(task);
        let status = if !actual_finish.is_empty() {
            "Complete"
        } else if !actual_start.is_empty() {
            "In progress"
        } else {
            "Not started"
        };

        let (baseline_start, baseline_finish, baseline_variance) = self.baseline_card(TaskId(task));
        Some(TaskDetails {
            task,
            name,
            outline_path,
            activity_id: self.task_code(task).to_string(),
            status: status.to_string(),
            actual_start,
            actual_finish,
            percent_complete: self.task_percent(task),
            start: self.date_text(dates.es),
            finish: self.date_text(dates.ef),
            late_start: self.date_text(self.schedule().late_start_shown(TaskId(task), dates)),
            late_finish: self.date_text(dates.lf),
            float_minutes: dates.float,
            float_days,
            float_hours,
            float_rem_minutes,
            float_text,
            critical: dates.is_critical(),
            predecessor_count,
            predecessors,
            successor_count,
            successors,
            baseline_start,
            baseline_finish,
            baseline_variance,
            resources: (self.resources(TaskId(task)).iter())
                .map(|a| a.name.clone())
                .collect(),
        })
    }

    /// The selected task and its cached driving path.
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Clear the selected task, its driving path and the selected link.
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_stale = false;
        self.links.selected = None;
    }

    /// Select `task` and build its driving path. Unknown tasks are ignored.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row indexes the timeline order, which holds at most one entry per task, so it is below the slot count; Schedule::add_task casts that count to u32 with no check against u32::MAX, so past 2^32 slots it wraps"
    )]
    pub fn select(&mut self, task: u32) {
        if task as usize >= self.schedule.tasks().len() {
            return;
        }
        let Some(row) = self.timeline().row_of_task(task) else {
            return;
        };
        self.selection = Some(self.build_selection(task, row as u32));
        self.range = None;
        self.selection_stale = false;
        self.links.selected = None;
    }

    /// Rebuild a selected path invalidated by a successful recompute.
    pub fn refresh_selection(&mut self) {
        self.prune_range();
        if !self.selection_stale {
            return;
        }
        let Some((task, row)) = self.selection.as_ref().map(|s| (s.task, s.row)) else {
            self.selection_stale = false;
            return;
        };
        self.selection = Some(self.build_selection(task, row));
        self.selection_stale = false;
    }

    /// The cached highlight used for dependency arrows.
    pub fn highlight(&self) -> Option<&timeline::scene::Highlight> {
        self.selection.as_ref().map(Selection::highlight)
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "a row indexes the timeline order, which holds at most one entry per task, so it is below the slot count; Schedule::add_task casts that count to u32 with no check against u32::MAX, so past 2^32 slots it wraps"
    )]
    pub(crate) fn sync_selection_row(&mut self) {
        let Some(task) = self.selection.as_ref().map(|selection| selection.task) else {
            return;
        };
        let row = self
            .timeline()
            .row_of_task(task)
            .map_or(timeline::HIDDEN_ROW, |row| row as u32);
        if let Some(selection) = self.selection.as_mut() {
            selection.row = row;
        }
    }

    fn build_selection(&mut self, task: u32, row: u32) -> Selection {
        self.chain_builds = self.chain_builds().wrapping_add(1);
        let (back, back_capped) = self.walk_chain(task, true);
        let (forward, forward_capped) = self.walk_chain(task, false);

        // A driving link is on the path when it joins two tasks of the same
        // half: the task and its drivers, or the task and what it drives.
        // (A driver of the task that also drives something the task drives
        // is not on the path through the task.)
        let n = self.schedule.tasks().len();
        let mut edges = Vec::new();
        for half in [&back, &forward] {
            let mut in_half = vec![false; n];
            in_half[task as usize] = true;
            for &t in half {
                in_half[t as usize] = true;
            }
            for pred in std::iter::once(task).chain(half.iter().copied()) {
                for succ in self
                    .schedule
                    .driven_successors(&self.solution, TaskId(pred))
                {
                    if in_half[succ.0 as usize] {
                        edges.push((pred, succ.0));
                    }
                }
            }
        }
        let subtree = self
            .outline
            .as_ref()
            .filter(|outline| outline.is_summary(TaskId(task)))
            .map(|outline| outline.subtree(TaskId(task)));
        if let Some(subtree) = subtree {
            // One pass over the link list: a link is inside the subtree when
            // both of its ends are.
            let mut in_subtree = vec![false; n];
            for descendant in subtree {
                in_subtree[descendant.0 as usize] = true;
            }
            edges.extend(
                self.schedule
                    .live_dependencies()
                    .map(|(_, d)| d)
                    .filter(|d| in_subtree[d.pred.0 as usize] && in_subtree[d.succ.0 as usize])
                    .map(|d| (d.pred.0, d.succ.0)),
            );
        }
        let tasks = std::iter::once(task)
            .chain(back.iter().copied())
            .chain(forward.iter().copied())
            .chain(subtree.into_iter().flatten().map(|task| task.0));
        let highlight = timeline::scene::Highlight::new(n, tasks, edges);
        Selection {
            task,
            row,
            back,
            back_capped,
            forward,
            forward_capped,
            highlight,
        }
    }

    pub(crate) fn walk_chain(&self, task: u32, backwards: bool) -> (Vec<u32>, bool) {
        let mut seen = vec![false; self.schedule.tasks().len()];
        seen[task as usize] = true;
        let mut level = vec![task];
        let mut chain = Vec::new();
        while !level.is_empty() {
            let mut next_level = Vec::new();
            for node in level {
                let next = if backwards {
                    self.schedule
                        .driving_predecessors(&self.solution, TaskId(node))
                } else {
                    self.schedule
                        .driven_successors(&self.solution, TaskId(node))
                };
                for next in next {
                    if !seen[next.0 as usize] {
                        next_level.push(next.0);
                    }
                }
            }
            next_level.sort_unstable();
            next_level.dedup();
            let remaining = CHAIN_CAP - chain.len();
            if next_level.len() > remaining {
                chain.extend(next_level.into_iter().take(remaining));
                return (chain, true);
            }
            for &next in &next_level {
                seen[next as usize] = true;
            }
            chain.extend_from_slice(&next_level);
            level = next_level;
        }
        (chain, false)
    }
}
