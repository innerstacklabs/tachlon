//! A press on a leaf bar's finish edge (gantt#1875): the drag sets the task's duration to the
//! working time from its start to the end of the day under the pointer
//! ([`Session::duration_to_day_end`]), and one undo puts the duration back.

use super::{Before, Press};
use crate::{Session, edit::Edit, texts as t};
use engine::TaskId;
use timeline::BarDrag;

impl Session {
    /// What a press on `task`'s bar remembers: its duration on the finish edge, else its start.
    pub(super) fn press_before(&self, task: u32) -> Before {
        let t = &self.schedule.tasks()[task as usize];
        match self.timeline().bar_drag() {
            Some(b) if b.edge => Before::Duration(t.duration),
            _ => Before::Snet(t.snet),
        }
    }

    /// One frame of a resize. A pointer left of the start asks for the start's own day. A task
    /// with actuals keeps its duration, and the drag says why in the calm note.
    pub(super) fn resize_frame(&mut self, b: BarDrag, press: Press) {
        let task = TaskId(b.task);
        let t = &self.schedule.tasks()[b.task as usize];
        if t.actual_start.is_some() || t.actual_finish.is_some() {
            let name = &self.meta[b.task as usize].name;
            let note = self.texts.say(&t::DRAG_RESIZE_STARTED, &[("name", name)]);
            return self.say_drag(note);
        }
        let current = t.duration;
        let Some(minutes) = self.wanted_duration(b, press) else {
            return;
        };
        let ms = if minutes == current {
            Some(0.0)
        } else if Before::Duration(minutes) == press.before {
            self.restore(press)
        } else {
            self.run(Edit::SetDuration { task, minutes })
                .ok()
                .map(|(_, ms)| ms)
        };
        if let Some(ms) = ms {
            self.record_drag(b.task, self.solution.dates[b.task as usize].es, ms);
        }
    }
}
