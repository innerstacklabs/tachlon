//! Read accessors for the private `Session` fields that outside code reads (gs#159).

use timeline::Timeline;

use crate::{DragInfo, Session};

impl Session {
    /// The timeline: viewport, pointer state and row order.
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// How many changes the schedule has had; a restored drag puts it back.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// How many changes the table has had to redraw for; it only goes up.
    pub fn stamp(&self) -> u32 {
        self.stamp
    }

    /// The last drag result, if there is one.
    pub fn drag_info(&self) -> Option<&DragInfo> {
        self.drag_info.as_ref()
    }

    /// How many driving-path cache builds there have been.
    pub fn chain_builds(&self) -> u32 {
        self.chain_builds
    }
}
