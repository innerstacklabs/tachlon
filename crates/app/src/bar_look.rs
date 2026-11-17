//! A task's own look on its bar: the color and the label a saved document gives it (gantt#1875).

use crate::Session;
use engine::TaskId;

impl timeline::scene::BarColors for Session {
    /// The task's color, always opaque: the scene mixes the body from it, so no alpha reaches it.
    fn bar_color(&self, task: u32) -> Option<[f32; 4]> {
        let [r, g, b, _] = self.meta.get(task as usize)?.color?;
        let [r, g, b] = [r, g, b].map(|v| f32::from(v) / 255.0);
        Some([r, g, b, 1.0])
    }
}

impl Session {
    /// The text on task `task`'s bar: its label, else its own name ([`Session::task_label`]), and
    /// an empty string if absent or deleted. A blank label counts as none. The table shows the name.
    #[must_use]
    pub fn bar_label(&self, task: u32) -> &str {
        let meta = self.meta.get(task as usize);
        match meta.and_then(|m| m.label.as_deref()) {
            Some(label) if self.is_live(TaskId(task)) && !label.trim().is_empty() => label,
            _ => self.task_label(task),
        }
    }
}
