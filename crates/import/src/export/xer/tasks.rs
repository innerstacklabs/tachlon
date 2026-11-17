//! The projects, `PROJWBS` rows and `TASK` identity cells of an XER file.
//!
//! The layout is worked out first (which tasks form which project, which WBS node each task sits
//! in) and then written. A summary is a task whose next live task has a deeper outline level, the
//! rule the MS Project writer uses.

use xer::{Cell, Writer};

use engine::{Instant, Limit, Minutes};

use super::constraint::{Constraints, constraints};
use super::rows::{absolute, cell_error};
use super::start::project_starts;
use super::{XerLayout, XerOptions};
use crate::activity_id::next_activity_id;
use crate::{ImportError, Imported};

const WBS_FIELDS: [&str; 7] = [
    "wbs_id",
    "proj_id",
    "parent_wbs_id",
    "seq_num",
    "proj_node_flag",
    "wbs_short_name",
    "wbs_name",
];
const TASK_FIELDS: [&str; 16] = [
    "task_id",
    "proj_id",
    "wbs_id",
    "clndr_id",
    "task_code",
    "task_name",
    "status_code",
    "target_drtn_hr_cnt",
    "remain_drtn_hr_cnt",
    "phys_complete_pct",
    "cstr_type",
    "cstr_date",
    "cstr_type2",
    "cstr_date2",
    "act_start_date",
    "act_end_date",
];

/// A project's `PROJECT` row: its name, the `clndr_id` of its first task and its start (an
/// instant from the schedule origin, set once every task is laid out).
pub(super) struct Project {
    pub(super) name: String,
    pub(super) calendar: Option<usize>,
    pub(super) start: Instant,
}

/// One `PROJWBS` row.
struct Node {
    project: usize,
    parent: Option<usize>,
    /// The number among the siblings, from 1.
    seq: usize,
    /// The node's whole WBS code, empty when it has none.
    code: String,
    short_name: String,
    name: String,
    children: usize,
}

/// One `TASK` row.
struct Activity {
    project: usize,
    node: usize,
    calendar: usize,
    code: String,
    name: String,
    /// Working minutes of its calendar.
    duration: Minutes,
    snet: Option<Instant>,
    progress: Option<u8>,
    /// The source task's actual start, actual finish and remaining work, as `ImportedTask`
    /// holds them.
    actual_start: Option<Instant>,
    actual_finish: Option<Instant>,
    remaining: Option<Minutes>,
    /// Whether the source task is under way or has an actual date, so its status is not
    /// `TK_NotStart`.
    started: bool,
    /// Whether the source task has a baseline date or a deadline, which the file does not hold.
    baseline: bool,
    deadline: bool,
    /// The source task's limit (a pin, a no-later-than, a finish-no-earlier-than or as late as
    /// possible), which the file holds in at most two constraint slots.
    limit: Limit,
}

/// The rows of an XER file's project, WBS and task tables. A project, node or task id is its
/// index plus one; the roots are the first nodes, one per project and in the same order.
pub(super) struct Layout {
    pub(super) projects: Vec<Project>,
    nodes: Vec<Node>,
    activities: Vec<Activity>,
}

impl Layout {
    /// With [`XerLayout::FromXer`], each level-1 summary is a project and a level-1 leaf joins
    /// the project before it (the first one when none comes before). Anything else, or a
    /// schedule with no level-1 summary, is one project named `options.name`.
    #[expect(
        clippy::too_many_lines,
        reason = "one linear outline walk that sets the projects; splitting it would only move the same locals into arguments"
    )]
    pub(super) fn new(imported: &Imported, options: &XerOptions) -> Layout {
        let schedule = &imported.schedule;
        let count = schedule.tasks().len().min(imported.tasks.len());
        let live: Vec<usize> = (0..count)
            .filter(|&index| !schedule.tasks()[index].deleted)
            .collect();
        let level = |position: usize| live.get(position).map(|&i| imported.tasks[i].outline_level);
        let summary = |position: usize| matches!((level(position), level(position + 1)), (Some(own), Some(next)) if next > own);
        let root = |position: usize| level(position) == Some(1) && summary(position);
        let from_xer = matches!(options.layout, XerLayout::FromXer) && (0..live.len()).any(root);

        let mut layout = Layout {
            projects: Vec::new(),
            nodes: Vec::new(),
            activities: Vec::new(),
        };
        if from_xer {
            for position in (0..live.len()).filter(|&position| root(position)) {
                let task = &imported.tasks[live[position]];
                layout.add_root(&task.name, task.wbs_code.as_deref());
            }
        } else {
            layout.add_root(&options.name, None);
        }

        let mut taken: Vec<String> = live
            .iter()
            .filter_map(|&index| imported.tasks[index].code.clone())
            .collect();
        let mut preceding: Option<String> = None;
        // The summaries still open as (outline level, node), and the project in force.
        let mut open: Vec<(u8, usize)> = Vec::new();
        let mut project = 0;
        let mut roots = 0;
        for (position, &index) in live.iter().enumerate() {
            let task = &imported.tasks[index];
            let calendar = schedule.tasks()[index].calendar.0 as usize + 1;
            if from_xer && root(position) {
                project = roots;
                roots += 1;
                open.clear();
                open.push((task.outline_level, project));
                layout.projects[project].calendar.get_or_insert(calendar);
                continue;
            }
            layout.projects[project].calendar.get_or_insert(calendar);
            while open
                .last()
                .is_some_and(|&(own, _)| own >= task.outline_level)
            {
                open.pop();
            }
            let parent = open.last().map_or(project, |&(_, node)| node);
            if summary(position) {
                let node = layout.add_node(parent, task.name.clone(), task.wbs_code.as_deref());
                open.push((task.outline_level, node));
                continue;
            }
            let code = if let Some(code) =
                task.code.as_deref().filter(|code| !code.trim().is_empty())
            {
                code.to_owned()
            } else {
                let code = next_activity_id(preceding.as_deref(), taken.iter().map(String::as_str));
                taken.push(code.clone());
                code
            };
            preceding = Some(code.clone());
            layout.activities.push(Activity {
                project,
                node: parent,
                calendar,
                code,
                name: task.name.clone(),
                duration: schedule.tasks()[index].duration,
                snet: schedule.tasks()[index].snet,
                progress: task.progress,
                actual_start: task.actual_start,
                actual_finish: task.actual_finish,
                remaining: task.remaining,
                started: task.in_progress
                    || task.actual_start.is_some()
                    || task.actual_finish.is_some(),
                baseline: task.baseline_start.is_some() || task.baseline_finish.is_some(),
                deadline: task.deadline.is_some(),
                limit: schedule.tasks()[index].limit,
            });
        }
        // A project with no task of its own takes the schedule's first calendar, as before.
        let fallback = schedule
            .tasks()
            .first()
            .map(|task| task.calendar.0 as usize + 1);
        for project in &mut layout.projects {
            project.calendar = project.calendar.or(fallback);
        }
        let starts = project_starts(
            layout.projects.len(),
            imported.project_start,
            layout.activities.iter().map(|a| (a.project, a.snet)),
        );
        for (project, start) in layout.projects.iter_mut().zip(starts) {
            project.start = start;
        }
        layout
    }

    fn add_root(&mut self, name: &str, code: Option<&str>) {
        let project = self.projects.len();
        self.projects.push(Project {
            name: name.to_owned(),
            calendar: None,
            start: 0,
        });
        let code = code.unwrap_or_default().to_owned();
        self.nodes.push(Node {
            project,
            parent: None,
            seq: 0,
            short_name: code.clone(),
            code,
            name: name.to_owned(),
            children: 0,
        });
    }

    /// A node under `parent`. Its short name is the part of its WBS code after the parent's, or
    /// the whole code when the parent's is not a prefix.
    fn add_node(&mut self, parent: usize, name: String, code: Option<&str>) -> usize {
        let code = code.unwrap_or_default().to_owned();
        let above = &self.nodes[parent];
        let short_name = code
            .strip_prefix(above.code.as_str())
            .and_then(|rest| rest.strip_prefix('.'))
            .map_or_else(|| code.clone(), str::to_owned);
        let project = above.project;
        self.nodes[parent].children += 1;
        let seq = self.nodes[parent].children;
        self.nodes.push(Node {
            project,
            parent: Some(parent),
            seq,
            code,
            short_name,
            name,
            children: 0,
        });
        self.nodes.len() - 1
    }

    /// Write the `PROJWBS` table.
    pub(super) fn write_wbs(&self, writer: &mut Writer) -> Result<(), ImportError> {
        let error = |error| cell_error("PROJWBS", error);
        writer.table("PROJWBS", &WBS_FIELDS).map_err(error)?;
        for (index, node) in self.nodes.iter().enumerate() {
            let id = (index + 1).to_string();
            let project = (node.project + 1).to_string();
            let parent = node.parent.map(|node| (node + 1).to_string());
            let seq = node.seq.to_string();
            let flag = if node.parent.is_none() { "Y" } else { "N" };
            writer
                .row(&[
                    Cell::Text(&id),
                    Cell::Text(&project),
                    Cell::Text(parent.as_deref().unwrap_or_default()),
                    Cell::Text(&seq),
                    Cell::Text(flag),
                    Cell::Text(&node.short_name),
                    Cell::Text(&node.name),
                ])
                .map_err(error)?;
        }
        Ok(())
    }

    /// List the baseline, the deadline and the limits of the tasks written, which the file does
    /// not hold: a task whose rules do not all fit the two constraint slots counts once.
    pub(super) fn list_unwritten(&self, left_out: &mut Vec<String>) {
        let counts = [
            (
                "baseline",
                self.activities.iter().filter(|a| a.baseline).count(),
            ),
            (
                "deadline",
                self.activities.iter().filter(|a| a.deadline).count(),
            ),
            (
                "constraint",
                self.activities
                    .iter()
                    .filter(|a| self.constraints(a).dropped)
                    .count(),
            ),
        ];
        for (what, n) in counts.into_iter().filter(|&(_, n)| n > 0) {
            let tasks = if n == 1 { "task" } else { "tasks" };
            left_out.push(format!("the {what} on {n} {tasks} was not written"));
        }
    }

    /// The constraint pairs of `activity`: its start-no-earlier-than only when it is above its
    /// project's start (the reader floors every task at that start anyway), and its limit.
    fn constraints(&self, activity: &Activity) -> Constraints {
        let start = self.projects[activity.project].start;
        constraints(activity.snet.filter(|&snet| snet > start), activity.limit)
    }

    /// Write the `TASK` table: identity, status, durations, percent, the two constraints and the
    /// actual dates. The target duration is the task's duration in hours. A task not started has the same
    /// remaining duration (the reader takes it); a started one has its remaining work, and a
    /// finished one none. Its status is `TK_Active` or `TK_Complete`, with `act_start_date` and
    /// `act_end_date`, so the reader gives the task back under way or finished. A task gets
    /// its rules as the primary and secondary constraints (`constraint.rs`).
    pub(super) fn write_tasks(
        &self,
        writer: &mut Writer,
        origin_day: i64,
    ) -> Result<(), ImportError> {
        let error = |error| cell_error("TASK", error);
        writer.table("TASK", &TASK_FIELDS).map_err(error)?;
        for (index, activity) in self.activities.iter().enumerate() {
            let id = (index + 1).to_string();
            let project = (activity.project + 1).to_string();
            let node = (activity.node + 1).to_string();
            let calendar = activity.calendar.to_string();
            let target = hours(activity.duration);
            let remaining = match (activity.actual_finish, activity.remaining) {
                (Some(_), _) => hours(0),
                (None, Some(left)) if activity.started => hours(left),
                (None, _) => target.clone(),
            };
            let status = match (activity.actual_finish, activity.started) {
                (Some(_), _) => "TK_Complete",
                (None, true) => "TK_Active",
                (None, false) => "TK_NotStart",
            };
            let percent = activity.progress.map(|value| value.to_string());
            let mut slots = [("", String::new()), ("", String::new())];
            for (slot, (kind, at)) in slots.iter_mut().zip(self.constraints(activity).pairs) {
                let date = at.map_or_else(|| Ok(String::new()), |at| absolute(origin_day, at))?;
                *slot = (kind, date);
            }
            let actual = |instant: Option<Instant>| {
                instant.map_or_else(|| Ok(String::new()), |at| absolute(origin_day, at))
            };
            let (actual_start, actual_end) = (
                actual(activity.actual_start)?,
                actual(activity.actual_finish)?,
            );
            writer
                .row(&[
                    Cell::Text(&id),
                    Cell::Text(&project),
                    Cell::Text(&node),
                    Cell::Text(&calendar),
                    Cell::Text(&activity.code),
                    Cell::Text(&activity.name),
                    Cell::Text(status),
                    Cell::Text(&target),
                    Cell::Text(&remaining),
                    Cell::Text(percent.as_deref().unwrap_or_default()),
                    Cell::Text(slots[0].0),
                    Cell::Text(&slots[0].1),
                    Cell::Text(slots[1].0),
                    Cell::Text(&slots[1].1),
                    Cell::Text(&actual_start),
                    Cell::Text(&actual_end),
                ])
                .map_err(error)?;
        }
        Ok(())
    }
}

/// Working minutes as hours, the unit the file's durations use.
#[expect(
    clippy::cast_precision_loss,
    reason = "the engine bounds a duration at MAX_DURATION (52,704,000 minutes), far below 2^53, so the f64 cast is exact"
)]
fn hours(minutes: Minutes) -> String {
    (minutes as f64 / 60.0).to_string()
}
