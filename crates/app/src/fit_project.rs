//! The Fit button (gs#703): the way back to the overview after zooming or scrolling away.

use crate::Session;
use engine::TaskId;

impl Session {
    /// Show the earliest early start of any live task to the project finish, with the margin
    /// [`timeline::Viewport::fit`] gives. The row filter and the collapsed summaries do not matter: the
    /// span is the whole schedule's. Returns false, changing nothing, with no live task. It ends the
    /// opening fit, as a wheel does, and leaves the rows where they are. The view eases there over
    /// [`crate::fly::FLY_MS`] (see [`Session::advance_fly`]), or jumps with reduced motion.
    pub fn fit_project(&mut self) -> bool {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the task slot count; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
        )]
        let first = (0..self.schedule.tasks().len() as u32)
            .filter(|&t| !self.schedule.is_deleted(TaskId(t)))
            .map(|t| self.solution.dates[t as usize].es)
            .min();
        let Some(first) = first else {
            return false;
        };
        let finish = self.solution.project_finish.max(first);
        let from = self.pose();
        #[expect(
            clippy::cast_precision_loss,
            reason = "an instant is minutes from the schedule origin; past 2^53 (about 17 billion years) f64 would round it, which a pixel position cannot show"
        )]
        self.fit(first as f64, finish as f64);
        self.fly_from(from);
        true
    }
}
