//! What the pointer is over, beyond the task.

use engine::TaskDates;

use crate::Timeline;

impl Timeline {
    /// Whether the pointer is on a leaf bar's finish edge (see [`Self::hit_edge`]).
    #[must_use]
    pub fn hovered_edge(&self, dates: &[TaskDates]) -> bool {
        self.pointer
            .is_some_and(|(x, y)| self.hit_edge(dates, x, y))
    }
}
