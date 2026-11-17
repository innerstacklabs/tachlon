//! A press on a finished task's bar (gs#895): a task with an actual finish is fixed at its
//! actuals, so the press is no [`super::Press`] and no drag frame edits anything. The release
//! says why the bar did not move.

use super::DRAG_THRESHOLD_PX;
use crate::{Session, texts as t};
use engine::TaskId;

impl Session {
    /// Whether `task` has an actual finish, which fixes it at its actual dates.
    pub(crate) fn is_finished(&self, task: TaskId) -> bool {
        self.schedule.tasks()[task.0 as usize]
            .actual_finish
            .is_some()
    }

    /// The pointer moved to (x, y): a finished task's press that has gone past the drag threshold
    /// is a bar drag as far as the cursor, the edits that wait for it and the release note go
    /// (all read `drag_started`), though it has no [`super::Press`].
    pub(super) fn finished_move(&mut self, x: f64, y: f64) {
        if let Some((_, at)) = self.finished_press
            && (x - at.0).hypot(y - at.1) >= DRAG_THRESHOLD_PX
        {
            self.drag_started = true;
        }
    }

    /// A drag of a finished task ended: say why the bar did not move. A press that never became a
    /// drag (a click) says nothing.
    pub(super) fn note_finished_drag(&mut self) {
        let Some((task, _)) = self.finished_press else {
            return;
        };
        if self.drag_started {
            let name = &self.meta[task as usize].name;
            let note = self.texts.say(&t::DRAG_FINISHED, &[("name", name)]);
            self.say_drag(note);
        }
    }
}
