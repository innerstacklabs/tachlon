//! The links a pin breaks, one flag per dependency id, kept for the `(revision, stamp)` they were
//! made at, so a frame reads them without a pass over the links. A schedule with no pin has none
//! to find and is not read. A drag makes a new solution every frame, so during a drag the frames
//! read the last flags kept; at the release the key has moved on and the next read is exact.

use crate::Session;
use std::sync::{Arc, Mutex, PoisonError};

type Kept = ((u32, u32), Arc<[bool]>);

/// The flags kept by a [`Session`]; a new session starts with none.
#[derive(Debug, Default)]
pub(crate) struct Memo(Mutex<Option<Kept>>);

impl Session {
    /// Whether `Schedule::pin_broken_links` returns each dependency id: the links drawn in `held`.
    pub fn pin_broken_marks(&self) -> Arc<[bool]> {
        let key = (self.revision(), self.stamp());
        let mut kept = self.broken.0.lock().unwrap_or_else(PoisonError::into_inner);
        match &mut *kept {
            Some((at, marks)) if *at == key || self.drag_started() => Arc::clone(marks),
            slot => {
                let mut marks = Vec::new();
                if self
                    .schedule()
                    .tasks()
                    .iter()
                    .any(|t| t.limit.pin.is_some())
                {
                    marks.resize(self.schedule().dependencies().len(), false);
                    for dep in self.schedule().pin_broken_links(self.solution()) {
                        marks[dep.0 as usize] = true;
                    }
                }
                let marks: Arc<[bool]> = marks.into();
                *slot = Some((key, Arc::clone(&marks)));
                marks
            }
        }
    }
}
