//! The links a pin breaks, one flag per dependency id, kept for the `(revision, stamp)` they were
//! made at, so a frame reads them without a pass over the links.

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
            Some((at, marks)) if *at == key => Arc::clone(marks),
            slot => {
                let mut marks = vec![false; self.schedule().dependencies().len()];
                for dep in self.schedule().pin_broken_links(self.solution()) {
                    marks[dep.0 as usize] = true;
                }
                let marks: Arc<[bool]> = marks.into();
                *slot = Some((key, Arc::clone(&marks)));
                marks
            }
        }
    }
}
