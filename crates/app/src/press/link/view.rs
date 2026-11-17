//! What the link gesture shows (gs#49 PR 4): the terminal and the preview line `add_link_overlay`
//! draws, the look they and the selected link take from the page's tokens, and the `link` key of
//! the status JSON. A child of `link.rs`, which owns the state it reads.

use crate::Session;
use timeline::Scene;
use timeline::scene::{
    PreviewFrom, PreviewTo, add_link_preview, add_link_terminal, bar_extent, span_extent,
};

/// The link look in CSS pixels: the tokens `--gantt-terminal-size`, `--gantt-terminal-gap`,
/// `--gantt-preview-w` and `--gantt-selected-w`. The shell passes the page's values with
/// `Session::set_link_look`; these are what a session draws until it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LinkLook {
    pub(super) terminal: f64,
    pub(super) gap: f64,
    pub(super) preview: f64,
    pub(super) selected: f64,
}

pub(super) const DEFAULT_LOOK: LinkLook = LinkLook {
    terminal: 8.0,
    gap: 3.0,
    preview: 2.0,
    selected: 2.0,
};

impl Session {
    /// Set the link look, in CSS pixels: the terminal's diameter and its gap from the bar, the
    /// drag preview line's thickness and the selected link's thickness. The diameter and the two
    /// thicknesses must be above 0 and the gap 0 or more; anything else, NaN included, is refused
    /// and changes nothing.
    ///
    /// # Errors
    /// A message naming the values when one is out of range.
    pub fn set_link_look(
        &mut self,
        terminal: f64,
        gap: f64,
        preview: f64,
        selected: f64,
    ) -> Result<(), String> {
        let above = |value: f64| value.is_finite() && value > 0.0;
        if !(above(terminal) && above(preview) && above(selected) && gap.is_finite() && gap >= 0.0)
        {
            return Err(format!(
                "link look {terminal}, {gap}, {preview}, {selected} is not sizes above 0 and a gap of 0 or more"
            ));
        }
        self.links.look = LinkLook {
            terminal,
            gap,
            preview,
            selected,
        };
        Ok(())
    }

    /// How far the link terminal reaches past a bar's finish end, CSS pixels: the gap plus the
    /// diameter. Bar names beside a bar start past it.
    pub fn terminal_reach(&self) -> f64 {
        self.links.look.gap + self.links.look.terminal
    }

    /// The selected link and its thickness, as the scene draws it.
    pub fn selected_link_drawn(&self) -> Option<(u32, f64)> {
        self.links
            .selected
            .map(|dep| (dep.0, self.links.look.selected))
    }

    /// The task whose terminal is drawn: the one on the hovered row, else the selected one.
    /// None while a press or a drag is in progress.
    fn terminal_task(&self) -> Option<u32> {
        if self.busy() {
            return None;
        }
        let tl = &self.timeline;
        let hovered = tl.hover_row().and_then(|row| tl.task_at_row(row));
        hovered
            .or(self.selection.as_ref().map(|selection| selection.task))
            .filter(|&task| self.leaf(task))
    }

    /// Draw the link drag's preview line over the bars, or, with no drag, the terminal of the
    /// hovered or selected bar. The preview is `link_refused` while a release would be refused.
    pub fn add_link_overlay(&self, scene: &mut Scene) {
        let tl = &self.timeline;
        let (v, dates) = (&tl.viewport, self.display_dates());
        let Some(drag) = self.links.drag.as_ref() else {
            if let Some(task) = self.terminal_task()
                && let Some(row) = tl.row_of_task(task)
            {
                let look = self.links.look;
                add_link_terminal(
                    scene,
                    v,
                    tl.dpr,
                    row,
                    &dates[task as usize],
                    look.terminal,
                    look.gap,
                    &self.theme,
                );
            }
            return;
        };
        let Some(row) = tl.row_of_task(drag.pred).filter(|_| drag.started) else {
            return;
        };
        let from = PreviewFrom {
            row,
            x: bar_extent(v, &dates[drag.pred as usize]).1,
        };
        let onto = drag.target.as_ref().and_then(|target| {
            let row = tl.row_of_task(target.task)?;
            let start = tl.summaries().get(target.task).map_or_else(
                || bar_extent(v, &dates[target.task as usize]).0,
                |(start, end)| span_extent(v, start, end).0,
            );
            Some((row, start, target.refusal.is_some()))
        });
        let (to, refused) = match onto {
            Some((row, x, refused)) => (PreviewTo::Bar(row, x), refused),
            None => (PreviewTo::Point(self.pointer.0, self.pointer.1), false),
        };
        let width = self.links.look.preview;
        add_link_preview(scene, v, tl.dpr, from, to, refused, width, &self.theme);
    }

    /// The `link` key of the status JSON: the selected link, the drag in progress, and why the
    /// last drag added nothing.
    pub(crate) fn link_status_json(&self) -> String {
        let text = |message: Option<&str>| message.map_or_else(|| "null".to_string(), crate::quote);
        let id = |value: Option<u32>| value.map_or_else(|| "null".to_string(), |v| v.to_string());
        let drag = match self.links.drag.as_ref().filter(|drag| drag.started) {
            Some(drag) => {
                let target = drag.target.as_ref();
                format!(
                    r#"{{"pred":{},"target":{},"refused":{}}}"#,
                    drag.pred,
                    id(target.map(|target| target.task)),
                    text(target.and_then(|target| target.refusal.as_deref())),
                )
            }
            None => "null".to_string(),
        };
        format!(
            r#"{{"selected":{},"drag":{drag},"refusal":{},"note":{}}}"#,
            id(self.links.selected.map(|dep| dep.0)),
            text(self.links.refusal.as_deref()),
            text(self.links.note.as_deref()),
        )
    }
}
