//! What the pointer asks of a dragged bar, which a drag frame applies and the before-edit function sees.

use super::{Before, Press};
use crate::{Session, edit::Edit};
use engine::TaskId;
use timeline::BarDrag;

impl Session {
    /// The start a move asks for; nothing starts before the origin (instant 0), where the calendar has no week.
    pub(super) fn wanted_start(&self, b: BarDrag, press: Press) -> i64 {
        let task = TaskId(b.task);
        let duration = self.schedule.tasks()[b.task as usize].duration;
        let wanted = b.wanted_start(&self.timeline().viewport).max(0);
        // A dropped task opens a working day of its calendar, so its
        // successors and summaries keep whole working days. A milestone keeps
        // the pointer's minute: it may also sit at a working-period end. So
        // does a pointer back at the press, which asks for no move: a start
        // the file gave mid-day is not an ask to open the day.
        if duration == 0 || wanted == press.es {
            wanted
        } else {
            self.schedule.calendar_of(task).day_start_at(wanted)
        }
    }

    /// The duration a resize asks for, `None` when the engine has none for the pointer's day.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the pointer's instant is minutes from the schedule origin, far inside i64"
    )]
    pub(super) fn wanted_duration(&self, b: BarDrag, press: Press) -> Option<i64> {
        let at = self.timeline().viewport.t_of(b.x).floor() as i64;
        self.duration_to_day_end(TaskId(b.task), at.max(press.es))
    }

    /// The edit a frame at the pointer would make.
    pub(crate) fn drag_edit(&self, press: Press) -> Option<Edit> {
        let b = self.timeline().bar_drag()?;
        let task = TaskId(b.task);
        Some(match press.before {
            Before::Duration(_) => Edit::SetDuration {
                task,
                minutes: self.wanted_duration(b, press)?,
            },
            Before::Snet(_) => Edit::Move {
                task,
                snet: Some(self.wanted_start(b, press)),
            },
        })
    }
}
