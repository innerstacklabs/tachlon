//! What moves if tasks slip a week, or one task takes days longer: the schedule is copied, each slipped
//! task gets a start-no-earlier-than of its early start plus 7 working days (or a longer duration), the
//! copy is solved, and the tasks whose dates differ are listed. The session is read, never changed.

use crate::edit::day_minutes;
use crate::{Session, json::quote};
use engine::TaskId;

/// The most tasks one ask takes.
pub const WHAT_MOVES_MAX: usize = 10;
const WEEK: i64 = 7;

/// `{"ok":false,"reason":kind,"message":message}`.
pub(crate) fn refuse(kind: &str, message: &str) -> String {
    format!(
        r#"{{"ok":false,"reason":"{kind}","message":{}}}"#,
        quote(message)
    )
}

impl Session {
    /// What moves if each of `tasks` slips a week, answered as JSON and without changing this
    /// session: each gets a start-no-earlier-than of its early start plus 7 working days of its
    /// calendar, and the copy is solved in full. A task named twice slips once. Answers
    /// `{"ok":true,"finish_was":s,"finish_now":s,"moved":[{"task","id","name","start_was","start_now",
    /// "finish_was","finish_now"}]}` (dates as the table writes them), or a
    /// refusal, `{"ok":false,"reason":r,"message":m}`, `r` being `busy`, `tooMany` (over
    /// [`WHAT_MOVES_MAX`]), `unknownTask`, `summary`, `hasActuals` or `other`.
    #[must_use]
    pub fn what_moves(&self, tasks: &[u32]) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        if tasks.len() > WHAT_MOVES_MAX {
            let message = format!("Ask about at most {WHAT_MOVES_MAX} tasks at a time.");
            return refuse("tooMany", &message);
        }
        let mut copy = self.schedule.clone();
        for &n in tasks {
            if let Some(refusal) = self.refusal(n) {
                return refusal;
            }
            let (task, es) = (TaskId(n), self.solution.dates[n as usize].es);
            let calendar = self.schedule.calendar_of(task);
            let snet = calendar.add(es, WEEK * day_minutes(calendar));
            if copy.set_snet(task, Some(snet)).is_err() {
                return refuse("other", "The task could not be slipped.");
            }
        }
        self.moved_by(copy)
    }

    /// What moves if `task` takes `days` working days of its calendar longer, answered like
    /// [`Session::what_moves`] (`tooMany` apart) and without changing this session.
    #[must_use]
    pub fn what_moves_longer(&self, task: u32, days: u32) -> String {
        if self.busy() {
            return refuse("busy", "Finish the drag first.");
        }
        if let Some(refusal) = self.refusal(task) {
            return refusal;
        }
        let task = TaskId(task);
        let longer = i64::from(days) * day_minutes(self.schedule.calendar_of(task));
        let duration = self.schedule.tasks()[task.0 as usize].duration;
        let mut copy = self.schedule.clone();
        if copy
            .set_duration(task, duration.saturating_add(longer))
            .is_err()
        {
            return refuse("other", "The task could not be made longer.");
        }
        self.moved_by(copy)
    }

    /// Why task `n` cannot be asked about: `unknownTask`, `summary` or `hasActuals`.
    pub(crate) fn refusal(&self, n: u32) -> Option<String> {
        self.refusal_of(n, true)
    }

    /// [`Session::refusal`], with `hasActuals` only when `started` refuses a task that has started.
    pub(crate) fn refusal_of(&self, n: u32, started: bool) -> Option<String> {
        let (task, i) = (TaskId(n), n as usize);
        if !self.is_live(task) {
            return Some(refuse("unknownTask", &format!("There is no task {n}.")));
        }
        let (name, t) = (&self.meta[i].name, &self.schedule.tasks()[i]);
        if self.outline.as_ref().is_some_and(|o| o.is_summary(task)) {
            let message = format!("\"{name}\" is a summary; ask about its sub-tasks.");
            return Some(refuse("summary", &message));
        }
        if started && (t.actual_start.is_some() || t.actual_finish.is_some()) {
            let message = format!("\"{name}\" has started: its dates are what happened.");
            return Some(refuse("hasActuals", &message));
        }
        None
    }

    /// The answer for `copy`, this schedule with the asked change made, solved in full.
    pub(crate) fn moved_by(&self, mut copy: engine::Schedule) -> String {
        match copy.compute() {
            Ok(solved) => self.solved_json(&solved),
            Err(_) => refuse("other", "The schedule did not solve."),
        }
    }

    /// The live tasks whose early start or finish differ in `solved`, a solved copy of this
    /// schedule, in task order.
    pub(crate) fn moved_tasks<'a>(
        &'a self,
        solved: &'a engine::Solution,
    ) -> impl Iterator<Item = TaskId> + 'a {
        let both = self.solution.dates.iter().zip(&solved.dates);
        (0..)
            .map(TaskId)
            .zip(both)
            .filter_map(|(task, (was, now))| {
                let same = was.es == now.es && was.ef == now.ef;
                (self.is_live(task) && !same).then_some(task)
            })
    }

    /// The `ok` answer for `solved`: the project finish before and after, and each moved task.
    pub(crate) fn solved_json(&self, solved: &engine::Solution) -> String {
        let q = |t| quote(&self.date_text(t));
        let moved: Vec<String> = self.moved_tasks(solved).map(|task| {
            let i = task.0 as usize;
            let (was, now) = (&self.solution.dates[i], &solved.dates[i]);
            let (id, name) = (quote(self.meta[i].code_text()), quote(&self.meta[i].name));
            let dates = [q(was.es), q(now.es), q(was.ef), q(now.ef)];
            format!(
                r#"{{"task":{i},"id":{id},"name":{name},"start_was":{},"start_now":{},"finish_was":{},"finish_now":{}}}"#,
                dates[0], dates[1], dates[2], dates[3]
            )
        }).collect();
        let (was, now) = (q(self.solution.project_finish), q(solved.project_finish));
        format!(
            r#"{{"ok":true,"finish_was":{was},"finish_now":{now},"moved":[{}]}}"#,
            moved.join(",")
        )
    }
}
