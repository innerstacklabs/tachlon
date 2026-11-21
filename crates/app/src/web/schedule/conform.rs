//! `LoadedSchedule.conform`: fit a schedule just read to what the host stores.

use super::LoadedSchedule;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl LoadedSchedule {
    /// Fit the schedule to the capability profile `profile` (`docs/WASM_API.md`), add a line to `unsupported` per kind of fit
    /// and solve it again. Throws a `JsError` for text that is not a profile.
    pub fn conform(&mut self, profile: &str) -> Result<(), JsError> {
        let (schedule, notes) = (&mut self.schedule, &mut self.unsupported);
        let fitted = crate::host::conform(schedule, &self.meta, notes, profile);
        if fitted.map_err(|e| JsError::new(&e))? {
            let solved = self.schedule.compute();
            self.solution = solved.map_err(|e| JsError::new(&e.to_string()))?;
            self.order = timeline::order_by_start(&self.solution.dates);
        }
        Ok(())
    }
}
