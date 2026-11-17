//! A frame that fails on purpose, so the shell's frame guard (`packages/gantt/src/frame-guard.ts`) can be
//! exercised by a test (gs#163). Plain Rust: each app keeps its own [`FrameFault`], so arming
//! one chart's next frame leaves every other chart on the page alone (gs#1083).

/// What the next frame of one app does on purpose: nothing, return an error, or panic.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FrameFault {
    /// The frame runs as usual.
    #[default]
    None,
    /// The frame returns an error; the app stays usable.
    Error,
    /// The frame panics; the module is then unusable.
    Panic,
}

impl FrameFault {
    /// Arm the next frame to fail, one shot. `panic` true panics (the module is then unusable);
    /// false returns an error (it is not).
    pub fn arm(&mut self, panic: bool) {
        *self = if panic { Self::Panic } else { Self::Error };
    }

    /// Called at the start of every frame: fails if armed, and disarms.
    ///
    /// # Errors
    ///
    /// Returns the deliberate error if the frame was armed with `panic` false.
    ///
    /// # Panics
    ///
    /// Panics on purpose if the frame was armed with `panic` true.
    #[expect(
        clippy::panic_in_result_fn,
        reason = "the panic is the fault a test arms"
    )]
    pub fn fire(&mut self) -> Result<(), &'static str> {
        match std::mem::take(self) {
            Self::Error => Err("deliberate frame error (test_fail_next_frame)"),
            Self::Panic => panic!("deliberate panic in frame (test_fail_next_frame)"),
            Self::None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrameFault;

    #[test]
    fn an_armed_fault_fires_once_and_only_where_it_was_armed() {
        let mut armed = FrameFault::default();
        let mut other = FrameFault::default();
        armed.arm(false);
        assert_eq!(other.fire(), Ok(()));
        assert!(armed.fire().is_err());
        assert_eq!(armed.fire(), Ok(()));
    }

    #[test]
    #[should_panic(expected = "deliberate panic in frame")]
    fn an_armed_panic_panics() {
        let mut fault = FrameFault::default();
        fault.arm(true);
        let _ = fault.fire();
    }
}
