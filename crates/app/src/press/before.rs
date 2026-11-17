//! What a press remembers of its task: the one value its drag changes, a start or a duration.
//! A release, a cancel and an undo put that value back, so a resize is no move in history.

use super::Press;
use crate::{Session, edit::Edit, texts as t};
use engine::{Schedule, TaskId};

/// The value of the pressed task that its drag changes, as it was at the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Before {
    /// The task's start-no-earlier-than: a move.
    Snet(Option<i64>),
    /// The task's duration in working minutes: a resize.
    Duration(i64),
}

impl Session {
    /// Put the dragged task back as it was at `press`: the schedule is the press-time one again,
    /// so the revision goes back to its value there, while `stamp` still moves so the table
    /// redraws (see [`Session::end_press`]). A duration the engine refuses to put back says so
    /// in the calm note.
    pub(super) fn restore(&mut self, press: Press) -> Option<f64> {
        let task = TaskId(press.task);
        let ms = match press.before {
            Before::Snet(snet) => self.constrain(task, snet)?,
            before @ Before::Duration(_) => match self.run(before.put_back(task)) {
                Ok((_, ms)) => ms,
                Err(error) => {
                    let name = &self.meta[press.task as usize].name;
                    let error = self.say(&error);
                    let note = self
                        .texts
                        .say(&t::DRAG_PUT_BACK, &[("name", name), ("error", &error)]);
                    self.say_drag(note);
                    return None;
                }
            },
        };
        self.revision = press.revision;
        Some(ms)
    }
}

impl Before {
    /// What `schedule` has for `task` now, of the kind this remembers.
    fn now(self, schedule: &Schedule, task: u32) -> Self {
        let task = &schedule.tasks()[task as usize];
        match self {
            Before::Snet(_) => Before::Snet(task.snet),
            Before::Duration(_) => Before::Duration(task.duration),
        }
    }

    /// Whether `task` still has what this remembers.
    pub(crate) fn held(self, schedule: &Schedule, task: u32) -> bool {
        self.now(schedule, task) == self
    }

    /// The edit that puts it back, which is also the undo step of the drag.
    pub(crate) fn put_back(self, task: TaskId) -> Edit {
        match self {
            Before::Snet(snet) => Edit::Move { task, snet },
            Before::Duration(minutes) => Edit::SetDuration { task, minutes },
        }
    }
}
