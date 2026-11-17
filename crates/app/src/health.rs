//! The tasks behind each count of the schedule checks report that walks tasks (`checks::report`): the
//! three link counts and the float, duration, date, resource, baseline and constraint counts.
//! The session is read, never changed.

use crate::{
    Session,
    check_filter::{CheckKind, LinkEnds},
    checks::schedule::{HIGH_DAYS, holds_a_date},
    edit::day_minutes,
    explain::refuse,
    json::quote,
};
use engine::TaskId;

/// The most tasks one count lists.
pub(crate) const HEALTH_MAX: usize = 50;

/// The keys of `checks::report::json` this answers, in this order.
const KEYS: [&str; 10] = [
    "missing_logic",
    "open_starts",
    "open_ends",
    "high_float",
    "high_duration",
    "invalid_dates",
    "missing_resources",
    "missed",
    "hard_constraints",
    "negative_float",
];

impl Session {
    /// For each of ten `checks::report` counts, the count and its first 50 tasks in task order, answered as
    /// `{"ok":true,"counts":{key:{"count":n,"tasks":[{"task","id","name"}]}}}`; `count` is the true total; a key is `null` when the
    /// report's is (`invalid_dates` with no data date, `missing_resources` with no resource); or a `"busy"` refusal.
    #[must_use]
    pub fn health(&self) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        let found = self.health_members();
        let off = |key: usize| {
            (key == 5 && self.data_date().is_none())
                || (key == 6 && self.resource_names().is_empty())
        };
        let counts: Vec<String> = KEYS
            .iter()
            .zip(&found)
            .enumerate()
            .map(|(key, (name, list))| {
                if off(key) {
                    return format!(r#""{name}":null"#);
                }
                let tasks: Vec<String> = list
                    .iter()
                    .take(HEALTH_MAX)
                    .map(|&n| self.health_task(n))
                    .collect();
                let count = list.len();
                format!(
                    r#""{name}":{{"count":{count},"tasks":[{}]}}"#,
                    tasks.join(",")
                )
            })
            .collect();
        format!(r#"{{"ok":true,"counts":{{{}}}}}"#, counts.join(","))
    }

    /// The slots each count in [`KEYS`] holds, in that order and in task order. A count that did not run
    /// (`invalid_dates`, `missing_resources`) holds none.
    pub(crate) fn health_members(&self) -> [Vec<u32>; 10] {
        let (schedule, solution) = (self.schedule(), self.solution());
        let ends = LinkEnds::of(schedule);
        let (data_date, resources) = (self.data_date(), !self.resource_names().is_empty());
        let baseline = self.baseline().zip(data_date);
        let mut found: [Vec<u32>; 10] = Default::default();
        for (index, task) in schedule.tasks().iter().enumerate() {
            if task.deleted {
                continue;
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let (slot, id) = (index as u32, TaskId(index as u32));
            let past =
                |at: Option<engine::Instant>| data_date.zip(at).is_some_and(|(date, at)| at > date);
            if past(task.actual_start) || past(task.actual_finish) {
                found[5].push(slot);
            }
            if self.outline().is_some_and(|outline| outline.is_summary(id)) {
                continue;
            }
            for (kind, list) in [
                CheckKind::MissingLogic,
                CheckKind::OpenStart,
                CheckKind::OpenEnd,
            ]
            .into_iter()
            .zip(&mut found)
            {
                if kind.holds(&ends, index, task) {
                    list.push(slot);
                }
            }
            let limit = HIGH_DAYS * day_minutes(schedule.calendar_of(id));
            let open = task.actual_finish.is_none();
            found[8].extend((open && holds_a_date(task)).then_some(slot));
            found[9].extend((open && solution.get(id).float < 0).then_some(slot));
            if open && solution.get(id).float > limit {
                found[3].push(slot);
            }
            if task.duration > limit {
                found[4].push(slot);
            }
            if resources
                && open
                && task.duration > 0
                && self.resources(id).iter().all(|a| a.name.is_empty())
            {
                found[6].push(slot);
            }
            let planned = baseline.and_then(|(base, date)| base.task(id).filter(|p| p.ef <= date));
            if planned.is_some_and(|p| task.actual_finish.is_none_or(|at| at > p.ef)) {
                found[7].push(slot);
            }
        }
        found
    }

    /// `{"task":n,"id":s,"name":s}` for the task in slot `n`.
    pub(crate) fn health_task(&self, n: u32) -> String {
        let meta = &self.meta[n as usize];
        format!(
            r#"{{"task":{n},"id":{},"name":{}}}"#,
            quote(meta.code_text()),
            quote(&meta.name)
        )
    }
}
