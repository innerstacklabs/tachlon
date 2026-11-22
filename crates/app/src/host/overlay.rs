//! A host's overlay dates (tl#2086): a second set of dates by key, drawn as a thin bar under the comparison bar.
//! Not an edit and not saved: no undo step, no commit, `revision` kept, and a new schedule starts with none.

use super::update::UpdateError;
use crate::Session;
use engine::TaskId;

impl Session {
    /// Draw `json`, records as [`Session::set_baseline_records`] reads them, as the overlay; `None` removes it.
    ///
    /// # Errors
    ///
    /// The [`UpdateError`] of the first record read wrong, as `set_baseline_records` names it; the overlay is then as it was.
    pub fn set_overlay_records(&mut self, json: Option<&str>) -> Result<(), UpdateError> {
        let overlay = json.map(|json| self.read_baseline_records(json));
        self.host.overlay = overlay.transpose()?;
        Ok(())
    }

    /// Add the overlay bars to `scene`, one slot under the comparison bars, lifted to stay inside the row. They read the records, not `display_dates`, so a
    /// slide or a drag does not move them. With no overlay it returns at once, before anything is allocated.
    pub fn add_overlay_bars(&self, scene: &mut timeline::Scene) {
        let Some(overlay) = &self.host.overlay else {
            return;
        };
        let tl = self.timeline();
        timeline::scene::add_comparison_bars(
            scene,
            &tl.viewport,
            tl.dpr,
            tl.summaries(),
            tl.rows(),
            |task| overlay.task(TaskId(task)).map(|t| (t.es, t.ef)),
            (1, self.theme().overlay, |task| {
                let base = self.baseline.as_ref();
                base.is_some_and(|b| b.task(TaskId(task)).is_some())
            }),
        );
    }
}
