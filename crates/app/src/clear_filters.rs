//! Clear filters (gs#703): the Filters panel's last button turns the row filter and the Lookahead
//! window off together.

use crate::{RowFilter, Session};

impl Session {
    /// Turn the row filter and the Lookahead window off, so the table lists every row again (collapse
    /// still applies). Returns true when there is an outline and no bar press is active, also when
    /// nothing was on; false, changing nothing, otherwise. One rebuild of the rows, and a change
    /// bumps [`Session::stamp`], so the task table redraws.
    pub fn clear_filters(&mut self) -> bool {
        if self.outline.is_none() || self.busy() {
            return false;
        }
        if !self.filtering() {
            return true;
        }
        self.row_filter = RowFilter::Off;
        self.lookahead = None;
        if !self.apply_rows() {
            self.stamp = self.stamp().wrapping_add(1);
        }
        true
    }
}
