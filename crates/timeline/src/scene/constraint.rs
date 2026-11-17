//! Which tasks the scene marks as held by a start or a finish constraint.

use std::fmt::Debug;

/// The app's answer to which tasks hold a real start or finish constraint
/// (`Backdrop::start_constraints`). It is asked once per drawn bar, never for every task, so a
/// frame costs the rows in view.
pub trait StartConstraints: Debug {
    /// Whether the bar of `task` takes the mark on its left edge.
    fn holds_start(&self, task: u32) -> bool;
    /// Whether the bar of `task` takes the mark on its right edge.
    fn holds_finish(&self, task: u32) -> bool;
}
