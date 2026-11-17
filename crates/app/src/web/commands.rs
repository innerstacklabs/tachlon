//! State-changing commands for the wasm app.

use super::App;
use crate::edit::{Edit, EditError, parse_duration_cell};
use engine::{DepId, TaskId};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl App {
    /// Toggle a summary task's collapsed state. A successful toggle bumps
    /// [`Session::stamp`], so the task table redraws.
    pub fn toggle_task(&mut self, i: u32) -> bool {
        self.session.toggle(TaskId(i))
    }

    /// Collapse the task outline to `level`.
    pub fn collapse_to_level(&mut self, level: u32) -> bool {
        self.session.collapse_to_level(level)
    }

    /// Expand every summary task.
    pub fn expand_all(&mut self) -> bool {
        self.session.expand_all()
    }

    /// Collapse the selected summary or select the selected leaf's parent.
    pub fn collapse_selected(&mut self) -> bool {
        self.session.collapse_selected()
    }

    /// Expand the selected summary when it is collapsed.
    pub fn expand_selected(&mut self) -> bool {
        self.session.expand_selected()
    }

    /// Expand the selected summary and every summary below it.
    pub fn expand_selected_branch(&mut self) -> bool {
        self.session.expand_selected_branch()
    }

    /// Select `task` from a click on its table row. False, changing nothing, during a bar press
    /// or drag, and for a task that is deleted, unknown or has no row.
    pub fn select_task(&mut self, task: u32) -> bool {
        self.session.select_task(task)
    }

    /// Move the selection `delta` visible rows (negative: up), stopping at the first or last row;
    /// collapsed children and filtered rows are skipped. Returns the selected task, or
    /// `undefined` during a bar press or drag or with no rows.
    pub fn move_selection(&mut self, delta: i32) -> Option<u32> {
        self.session.move_selection(delta)
    }

    /// Move the range's extent `delta` visible rows (negative: up), stopping at the first or last
    /// row; the selected task stays the anchor. Starts a range at the selected row, or at the
    /// first row with nothing selected. Returns the extent task, or `undefined` during a bar
    /// press or drag or with no rows.
    pub fn extend_selection(&mut self, delta: i32) -> Option<u32> {
        self.session.extend_selection(delta)
    }

    /// Make the selected row a range of that one row, or keep the range there is. False, changing
    /// nothing, with nothing selected, during a bar press or drag, or while the selection has no row.
    pub fn select_row_range(&mut self) -> bool {
        self.session.select_row_range()
    }

    /// Extend the range from the selected task to `task`, from a Shift+click on its table row.
    /// With nothing selected it selects `task` as a range of one row. False, changing nothing,
    /// during a bar press or drag, and for a task that is deleted, unknown or has no row.
    pub fn extend_to(&mut self, task: u32) -> bool {
        self.session.extend_to(task)
    }

    /// Set task `i`'s duration from table text.
    pub fn set_duration(&mut self, i: u32, text: &str) -> Result<(), JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        let task = TaskId(i);
        if self.session.schedule().tasks().get(i as usize).is_none() {
            return Err(self.thrown(&EditError::UnknownTask(task)));
        }
        let minutes = parse_duration_cell(text, self.session.schedule(), task)
            .map_err(|error| self.thrown(&error))?;
        self.session
            .edit(Edit::SetDuration { task, minutes })
            .map_err(|error| self.thrown(&error))
    }

    /// Rename the last outline segment of task `i`.
    pub fn rename_task(&mut self, i: u32, name: &str) -> Result<(), JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        self.session
            .edit(Edit::Rename {
                task: TaskId(i),
                name: name.trim().to_string(),
            })
            .map_err(|error| self.thrown(&error))
    }

    /// Insert a task named `New task` below the selected row's whole subtree, at the selection's
    /// level (with nothing selected, the last top-level row), and select it. Returns the new task.
    /// Throws `JsError` with the reason when refused; nothing changes then.
    pub fn insert_task(&mut self) -> Result<u32, JsError> {
        self.session
            .insert_below_selection()
            .map(|task| task.0)
            .map_err(|error| self.thrown(&error))
    }

    /// Delete the selected leaf task, recorded for undo. The selection moves to the next visible
    /// row's task, else the previous, else none. Returns the deleted task. Throws `JsError` with
    /// the reason when refused (nothing selected, a summary, a bar press); nothing changes then.
    pub fn delete_selected(&mut self) -> Result<u32, JsError> {
        self.session
            .delete_selected()
            .map(|task| task.0)
            .map_err(|error| self.thrown(&error))
    }

    /// Indent the selected row: it becomes the last child of its previous sibling, which opens if
    /// it was collapsed. One undo step. Throws `JsError` with the reason when refused (nothing
    /// selected, no row above to indent under, a row filter or lookahead window, a bar press, a
    /// link that would turn the row above into a summary); nothing changes then.
    pub fn indent_selected(&mut self) -> Result<(), JsError> {
        self.session
            .indent_selected()
            .map(|_| ())
            .map_err(|error| self.thrown(&error))
    }

    /// Outdent the selected row: it moves to just after its parent and adopts the siblings below
    /// it as its last children. One undo step, and undo hands the siblings back. Throws `JsError`
    /// with the reason when refused (nothing selected, already at the top level, a row filter or
    /// lookahead window, a bar press, a link that would turn the row into a summary); nothing
    /// changes then.
    pub fn outdent_selected(&mut self) -> Result<(), JsError> {
        self.session
            .outdent_selected()
            .map(|_| ())
            .map_err(|error| self.thrown(&error))
    }

    /// The selected link's id, or `undefined` when none is selected.
    pub fn selected_link(&self) -> Option<u32> {
        self.session.selected_link().map(|dep| dep.0)
    }

    /// Select the next (`forward`) or previous link of the selected task. False when there is
    /// none to step to.
    pub fn step_link(&mut self, forward: bool) -> bool {
        self.session.step_link(forward)
    }

    /// Remove the selected link, recorded for undo. Returns the removed link's id. Throws
    /// `JsError` with the reason when refused (no link selected, a drag in progress); nothing
    /// changes then.
    pub fn remove_selected_link(&mut self) -> Result<u32, JsError> {
        self.session
            .remove_selected_link()
            .map(|dep| dep.0)
            .map_err(|error| self.thrown(&error))
    }

    /// Set link `dep`'s type and lag from `text`: `FS`, `SS`, `FF` or `SF`, then an optional signed
    /// lag in duration text (`SS+2d`, `FF-1d`, `FS`). One undo step. Throws `JsError` with the
    /// reason, changing nothing, for text that does not parse, a removed or unknown link, a link
    /// that would repeat another or close a loop, or a bar or link drag in progress.
    pub fn set_link(&mut self, dep: u32, text: &str) -> Result<(), JsError> {
        self.session
            .set_link_text(DepId(dep), text)
            .map_err(|error| self.thrown(&error))
    }

    /// Link `dep`'s type and lag as text, such as `SS+2d`, `FF-1d` or `FS` (no lag), the lag in days
    /// of the calendar it is counted in; `App.set_link` reads the same text back. `undefined` for a
    /// removed or unknown link.
    pub fn link_text(&self, dep: u32) -> Option<String> {
        self.session.link_text(DepId(dep))
    }

    /// Make task `task` follow exactly the tasks `text` lists: Activity IDs with an optional link type
    /// and lag (`3, 5SS+1d`), as one undo step. Throws `JsError` with a sentence naming the item,
    /// changing nothing, for an item that is not a task, is `task` itself, repeats, or has bad link
    /// text, and for a summary, a loop or a bar or link drag in progress.
    pub fn set_predecessors_text(&mut self, task: u32, text: &str) -> Result<(), JsError> {
        self.session
            .set_predecessors_text(TaskId(task), text)
            .map_err(|error| self.thrown(&error))
    }

    /// The tasks `task` follows as `App.set_predecessors_text` takes them (`3, 5 SS+1d`), `undefined`
    /// for a task that is not live or follows one with no Activity ID.
    pub fn predecessors_text(&self, task: u32) -> Option<String> {
        self.session.predecessors_text(TaskId(task))
    }

    /// Whether a link drag is in progress (the shell sets the cursor).
    pub fn linking(&self) -> bool {
        self.session.linking()
    }

    /// Set the link look from `--gantt-terminal-size`, `--gantt-terminal-gap`, `--gantt-preview-w`
    /// and `--gantt-selected-w`, in CSS pixels. A size that is not above 0, a gap below 0 or a NaN
    /// throws and changes nothing.
    pub fn set_link_look(
        &mut self,
        terminal: f64,
        gap: f64,
        preview: f64,
        selected: f64,
    ) -> Result<(), JsError> {
        self.session
            .set_link_look(terminal, gap, preview, selected)
            .map_err(|error| JsError::new(&error))
    }

    /// Tell the app whether the page asked for no motion (`prefers-reduced-motion: reduce`). On,
    /// an edit starts no slide: the bars are at their new places on the first frame. A change
    /// drops the slide that runs. The setting survives `App.open`.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.session.set_reduced_motion(on);
    }

    /// Set the month and weekday names the app writes: `{"months":[12 names],"weekdays":[7 names]}`,
    /// January and Monday first. They name the months in the toolbar's baseline date, the what-if
    /// finish dates and the off-screen note, and survive `App.open`. Throws `JsError`, changing
    /// nothing, for text that is not that shape.
    pub fn set_locale(&mut self, json: &str) -> Result<(), JsError> {
        self.session
            .set_locale(json)
            .map_err(|error| JsError::new(&error))
    }

    /// Add TrueType or OpenType font-file bytes and set the text locale. Throws when a face cannot
    /// be read; readable faces stay added, labels reshape, and the fonts survive `App.open`.
    pub fn set_fonts(
        &mut self,
        locale: &str,
        faces: &web_sys::js_sys::Array,
    ) -> Result<(), JsError> {
        let given = faces.length() as usize;
        let faces = faces
            .iter()
            .map(|v| web_sys::js_sys::Uint8Array::new(&v).to_vec())
            .collect();
        if self.renderer.add_fonts(locale, faces) < given {
            return Err(JsError::new(
                "a font is not a font file the engine can read",
            ));
        }
        Ok(())
    }

    /// The sentences an edit is refused in, English, as JSON from each key to its text, for a host to
    /// translate and hand `set_locale` as `texts`. A `{name}`-style hole is filled by the app.
    pub fn default_texts() -> String {
        crate::english_json()
    }

    /// Tell the app whether the pointer is over the "what moved" note. While it is, the report
    /// stays at full opacity; on release it keeps the time it had left and at least one second of
    /// full opacity, then fades. Does nothing with no report or after it has faded. A change that
    /// clears the report, and `App.open`, end the hold.
    pub fn set_moved_hold(&mut self, on: bool) {
        self.session.set_moved_hold(on);
    }

    /// Set the baseline: a snapshot of every live task's dates as they are now. Returns the number
    /// of tasks it holds. One undo step. Throws `JsError`, changing nothing, during a bar press.
    pub fn set_baseline(&mut self) -> Result<u32, JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        self.session
            .edit(Edit::SetBaseline { baseline: None })
            .map_err(|error| self.thrown(&error))?;
        Ok(self.baseline_rows())
    }

    /// Clear the baseline. One undo step. Throws `JsError`, changing nothing, when there is none
    /// or during a bar press.
    pub fn clear_baseline(&mut self) -> Result<(), JsError> {
        if self.session.busy() {
            return Err(self.busy_error());
        }
        self.session
            .edit(Edit::ClearBaseline)
            .map_err(|error| self.thrown(&error))
    }

    /// Move the data date to 00:00 on `text`, a date written `YYYY-MM-DD`, and recompute; empty
    /// `text` clears the data date. One undo step. Throws `JsError`, changing nothing, for text
    /// that is not a date or during a bar press.
    pub fn set_data_date(&mut self, text: &str) -> Result<(), JsError> {
        self.session
            .set_data_date_text(text)
            .map_err(|error| self.thrown(&error))
    }

    /// Record task `i`'s progress as one undo step: `actual_start` and `actual_finish` are days
    /// written `YYYY-MM-DD`, `remaining` is a duration as `set_duration` takes it; empty text
    /// clears that field. Throws `JsError`, changing nothing, for an unknown task, text that does
    /// not parse, a finish before the start, a summary, or a bar press.
    pub fn set_progress(
        &mut self,
        i: u32,
        actual_start: &str,
        actual_finish: &str,
        remaining: &str,
    ) -> Result<(), JsError> {
        self.session
            .set_progress_text(TaskId(i), actual_start, actual_finish, remaining)
            .map_err(|error| self.thrown(&error))
    }

    /// Undo the latest edit. True means an edit was undone.
    pub fn undo(&mut self) -> Result<bool, JsError> {
        self.session.undo().map_err(|error| self.thrown(&error))
    }

    /// Redo the latest undone edit. True means an edit was redone.
    pub fn redo(&mut self) -> Result<bool, JsError> {
        self.session.redo().map_err(|error| self.thrown(&error))
    }

    /// The commits made since the last call, one per undo step, as a JSON string; the outbox is empty after.
    pub fn take_commits(&mut self) -> String {
        self.session.take_commits()
    }
}
