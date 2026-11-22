//! A press of Undo or Redo drops a step the engine refuses and goes on to the next (tl#2087).

use super::{History, Source, task_of};
use crate::texts::{STEP_GONE, STEP_GONE_MANY};
use crate::{Session, edit::EditError, host::Plan};

/// `applied` says a step ran; `dropped` counts refused steps. `report` is replaceable text,
/// present only when steps were dropped and a step then applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pressed {
    pub applied: bool,
    pub dropped: u32,
    pub report: Option<String>,
}

impl History {
    fn drop_top(&mut self, source: Source) {
        if source == Source::Undo {
            self.undo.pop_back();
            let len = self.undo.len();
            if let Some(mark) = &mut self.whatif {
                mark.depth = mark.depth.map(|depth| depth.min(len));
            }
        } else {
            self.redo.pop();
        }
    }
}

impl Session {
    /// Undo with the dropped-step count and report.
    /// # Errors
    /// As [`Session::undo`].
    pub fn undo_counted(&mut self) -> Result<Pressed, EditError> {
        self.press(Source::Undo, |s| {
            s.gate(&Plan::Replay("undo", Source::Undo))
        })
    }

    /// Redo with the dropped-step count and report.
    /// # Errors
    /// As [`Session::undo`].
    pub fn redo_counted(&mut self) -> Result<Pressed, EditError> {
        self.press(Source::Redo, |s| {
            s.gate(&Plan::Replay("redo", Source::Redo))
        })
    }

    /// A press of `source`: `gate` runs before each step tried. A refusal about the rows as they are
    /// now (`EditError::is_data`) drops the step and the press goes on; any other refusal, the gate's
    /// and a host's save refusal included, is returned and the step stays.
    pub(super) fn press(
        &mut self,
        source: Source,
        gate: impl Fn(&Self) -> Result<(), EditError>,
    ) -> Result<Pressed, EditError> {
        let mut dropped = 0u32;
        loop {
            gate(self)?;
            let step = match source {
                Source::Undo => self.undo_step(),
                _ => self.redo_step(),
            };
            match step {
                Err(error) if error.is_data() => {
                    self.history.drop_top(source);
                    dropped = dropped.saturating_add(1);
                }
                Ok(false) if dropped > 0 => return Err(EditError::StepGone),
                done => {
                    return done.map(|applied| {
                        let report = (dropped > 0 && applied).then(|| {
                            self.texts.say_by(
                                dropped == 1,
                                &STEP_GONE,
                                &STEP_GONE_MANY,
                                &[("count", &dropped.to_string())],
                            )
                        });
                        Pressed {
                            applied,
                            dropped,
                            report,
                        }
                    });
                }
            }
        }
    }

    fn redo_step(&mut self) -> Result<bool, EditError> {
        if self.busy() {
            return Ok(false);
        }
        let Some(entry) = self.history.redo.last().cloned() else {
            return Ok(false);
        };
        let row = task_of(&entry);
        let (inverse, _) = self.run_entry(entry, true)?;
        self.commit(inverse, Source::Redo);
        self.reveal_changed(row);
        Ok(true)
    }
}
