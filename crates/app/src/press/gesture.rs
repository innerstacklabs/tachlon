//! What the pointer is doing (gantt#1152), derived from the fields a press already sets.

use super::Before;
use crate::Session;

/// What the pointer is doing ([`Session::gesture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Idle,
    BarPressed,
    BarDragging,
    LinkPressed,
    LinkDragging,
    /// A press on a finished task's bar, which cannot move.
    FinishedPressed,
    /// A press on a leaf bar's finish edge, which sets its duration.
    ResizePressed,
    ResizeDragging,
}

impl Gesture {
    /// The name `Status.gesture` carries.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Gesture::Idle => "idle",
            Gesture::BarPressed => "bar-pressed",
            Gesture::BarDragging => "bar-dragging",
            Gesture::LinkPressed => "link-pressed",
            Gesture::LinkDragging => "link-dragging",
            Gesture::FinishedPressed => "finished-pressed",
            Gesture::ResizePressed => "resize-pressed",
            Gesture::ResizeDragging => "resize-dragging",
        }
    }
}

impl Session {
    /// What the pointer is doing now. Anything but [`Gesture::Idle`] keeps the keys, the
    /// editors and a second press out until the press ends.
    pub fn gesture(&self) -> Gesture {
        if self.link_down() {
            return if self.linking() {
                Gesture::LinkDragging
            } else {
                Gesture::LinkPressed
            };
        }
        if let Some(press) = self.press {
            let resize = matches!(press.before, Before::Duration(_));
            return match (resize, self.drag_started) {
                (false, true) => Gesture::BarDragging,
                (false, false) => Gesture::BarPressed,
                (true, true) => Gesture::ResizeDragging,
                (true, false) => Gesture::ResizePressed,
            };
        }
        if self.finished_press.is_some() {
            return Gesture::FinishedPressed;
        }
        Gesture::Idle
    }
}
