//! Keep the place in the rows (tl#703): when collapse, expand, a filter or Clear filters changes
//! which rows are visible, the table keeps showing the row at its top instead of the same pixel
//! offset, which after the change is some other part of the schedule.
//!
//! It runs once per change of the row order, never per frame. A row the change hides is replaced
//! by the nearest visible row at or before it in the outline: for a collapse, the collapsed
//! summary that now holds it.

use crate::Session;

/// The row at the top of the table before the rows change.
pub(crate) struct Place {
    /// The task on that row.
    task: u32,
    /// How far the table is scrolled into that row, in pixels.
    offset: f64,
}

impl Session {
    /// The task at the top of the table and the pixels scrolled into its row, `None` with no rows.
    pub(crate) fn place(&self) -> Option<Place> {
        let viewport = &self.timeline.viewport;
        let row = viewport.visible_rows().start;
        let task = self.timeline.task_at_row(row)?;
        Some(Place {
            task,
            offset: viewport.scroll_y.rem_euclid(viewport.row_height),
        })
    }

    /// Scroll so `place`'s row is at the top again, now that the rows have changed. A task the
    /// change hid gives way to the nearest visible row before it in the outline, at the top.
    pub(crate) fn keep_place(&mut self, place: Option<Place>) {
        let (Some(place), Some(outline)) = (place, self.outline.as_ref()) else {
            return;
        };
        let mut row = 0;
        let mut offset = 0.0;
        for &task in outline.preorder() {
            let here = self.timeline.row_of_task(task.0);
            if let Some(at) = here {
                row = at;
            }
            if task.0 == place.task {
                if here.is_some() {
                    offset = place.offset;
                }
                break;
            }
        }
        let viewport = &mut self.timeline.viewport;
        viewport.scroll_to_row(row);
        viewport.scroll_by(offset);
    }
}
