//! The Start constraint row filter (gs#703 K5): the tasks that carry a real start constraint
//! ([`Session::start_constraint`]), and the summaries above them.

use crate::{RowFilter, Session};

impl Session {
    /// Whether the Start constraint row filter is on.
    pub fn start_constraint_filter(&self) -> bool {
        self.row_filter == RowFilter::StartConstraint
    }

    /// Turn the Start constraint row filter on or off. Returns false when there is no outline or a
    /// bar press is active.
    pub fn set_start_constraint_filter(&mut self, on: bool) -> bool {
        let wanted = if on {
            RowFilter::StartConstraint
        } else if self.row_filter == RowFilter::StartConstraint {
            RowFilter::Off
        } else {
            self.row_filter.clone()
        };
        self.set_row_filter(wanted)
    }
}
