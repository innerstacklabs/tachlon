import { reason } from "./boot.js";
import { type HoverCard } from "./card.js";
import { type Cursor } from "./cursor.js";
import { type KeyRequest } from "./events.js";
import { type LinkEdit } from "./link-edit.js";
import { type RowEdit } from "./row-edit.js";
import { listening, type Shell } from "./shell.js";
import { type Split } from "./split.js";
import { type TaskDialog } from "./task-dialog.js";

/** The wheel delta one `+` or `-` sends with ctrl: equal to the toolbar's ZOOM_BUTTON_DY, so a key and a click zoom the same step. */
const ZOOM_KEY_DY = 350;
/** The keys a browser scrolls the page with. */
const SCROLL_KEYS = new Set(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"]);

/**
 * The chart's keys. The find, status, open and keys keys are the host's: `request` fires their event (`find`, `toggle-status`, `choose-file`,
 * `toggle-keys`) and says whether a handler heard it.
 */
export function installKeys( // oxlint-disable-line max-lines-per-function -- existing, over cap
  shell: Shell,
  request: (name: KeyRequest) => boolean,
  rows: RowEdit,
  cursor: Cursor,
  card: () => HoverCard,
  split: Split,
  links: LinkEdit,
  save: () => Promise<void>,
  taskDialog: TaskDialog,
): void {
  const { app, root, canvas, status } = shell;
  // An Alt-drag ends with the Alt key coming up, and a browser then moves focus to its menu bar: the chart keeps it.
  // The listeners are on the host's element: `listening` takes them off when the chart goes.
  listening(shell, root, "keyup", (e: KeyboardEvent) => {
    if (e.key === "Alt") e.preventDefault();
  });
  // Keys act only while focus is inside the chart (`docs/ARCHITECTURE.md`, Embedding, rule 2): the listener is on the root, which holds focus
  // when nothing inside it does, and nothing listens on the window or the document.
  listening(shell, root, "keydown", (e: KeyboardEvent) => { // oxlint-disable-line max-lines-per-function -- existing, over cap
    // The project picker is a dialog inside the root, so its keys bubble here: they are the dialog's, not the chart's.
    if (e.target instanceof Element && e.target.closest("dialog") !== null) return;
    // A key ends a Fit or find flight where it is (gs#309); the `f` below starts a new one. A key typed in a text field (the
    // Find box: its Enter starts a flight and bubbles here) is the field's, not the chart's.
    if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) app.cancel_fly();
    // Where the keyboard is (mode.ts): a cell being edited or a bar being dragged leaves the chart's own keys alone.
    const mode = cursor.mode();
    const editing = mode === "cell-edit";
    const gesture = status().gesture;
    const idle = !editing && gesture === "idle";
    const findKey = (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f";
    // Open a file: the chart's own key, like Ctrl+F; the browser's Open command is not taken while focus is outside the chart.
    const openKey = (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "o";
    // Save: the open schedule is downloaded as a document (never written over a file). The browser's own Save-page is taken
    // while focus is in the chart; a cell being edited or a bar being dragged saves nothing and says why in the alert line.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (!idle) rows.hint("Nothing saved yet. Press Enter to finish the edit, then save.");
      else {
        save().catch((err) => {
          console.warn(`gantt: save failed: ${reason(err)}`);
          rows.refuse(`Not saved: ${reason(err)}`);
        });
      }
      return;
    }
    if (openKey && idle && request("choose-file")) {
      e.preventDefault();
      return;
    }
    // With no handler the key is left to the browser, whose own find then opens.
    if (findKey && !editing && request("find")) {
      e.preventDefault();
      return;
    }
    if (e.target instanceof HTMLInputElement) return;
    // A press is down (a bar, a link or a finished task's bar): the keys that scroll the host page are taken and do nothing, so
    // the page stays where the pointer is. Escape still cancels the gesture below.
    if (gesture !== "idle" && SCROLL_KEYS.has(e.key)) {
      e.preventDefault();
      return;
    }
    // The pinned card (gs#703): Alt+Enter shows the selected row's card under the row; Escape or any other key hides it (a bare
    // modifier is not a key press, so the Alt of Alt+Enter leaves it). Escape goes on to its own commands below.
    const shownCard = card();
    const selected = status().selected;
    if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.key === "Enter" && idle) {
      e.preventDefault();
      if (selected !== null) shownCard.pin(selected.task);
      return;
    }
    if (!["Alt", "Shift", "Control", "Meta"].includes(e.key)) shownCard.unpin();
    // Select all rows (gs#703): Ctrl+A and Cmd+A make the visible rows a range, for Ctrl+C. In the find box or a cell editor (the
    // input-target return above) the key selects the text as before.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "a" && idle) {
      if (cursor.selectAll()) e.preventDefault();
      return;
    }
    // The list divider's arrows, with focus on the divider: they go through this one listener (embedding rule 2).
    if (e.target === split.handle && split.key(e)) return;
    const key = e.key.toLowerCase();
    const undo = (e.ctrlKey || e.metaKey) && !e.altKey && key === "z" && !e.shiftKey;
    const redo = (e.ctrlKey || e.metaKey) && !e.altKey && ((key === "z" && e.shiftKey) || (key === "y" && !e.shiftKey));
    if (undo || redo) {
      // The chart owns undo while it has focus: during a drag the key does nothing, and the browser must not
      // run its own undo either (it would return focus to the find box's last edit).
      e.preventDefault();
      if (!idle) return;
      try {
        if (undo) app.undo();
        else app.redo();
      } catch (err) {
        if (app.read_only()) rows.refuse(err);
        else console.warn(`gantt: ${undo ? "undo" : "redo"} failed: ${reason(err)}`);
      }
      return;
    }
    // A selected link owns Enter (gs#703): it opens the link editor ahead of the cell cursor's Enter. A focused button or link
    // keeps its own Enter, as the cell cursor leaves it.
    const control = e.target instanceof Element && e.target.closest("button, a, select, textarea, [contenteditable]") !== null;
    if (e.key === "Enter" && !control && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && (mode === "chart" || mode === "grid") && links.begin()) {
      e.preventDefault();
      return;
    }
    // Shift+Enter (gantt#1875) opens the selected task's fields in one dialog (task-dialog.ts); a plain Enter is the cursor's below.
    if (e.key === "Enter" && e.shiftKey && !control && !e.ctrlKey && !e.metaKey && !e.altKey && idle && selected !== null) {
      e.preventDefault();
      taskDialog.open(selected.task);
      return;
    }
    // The cell cursor's editing keys (gs#421), ahead of every plain key below: while the cursor is in the grid a printable key
    // edits the cell (or, on a cell that cannot be edited, does nothing), so `c`, `C`, `L`, `s`, `0` to `9`, `[` and `]` reach
    // no chart command until Escape or a press on the timeline puts the cursor away.
    if (idle && cursor.key(e)) {
      e.preventDefault();
      return;
    }
    // The keys list (gs#703): `?` opens it. After the cell cursor's keys, so in the list `?` types into the cell.
    if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey && idle) {
      e.preventDefault();
      request("toggle-keys");
      return;
    }
    if (!e.ctrlKey && !e.metaKey && !e.altKey && /^[0-9]$/.test(e.key) && idle) {
      // `1` to `9` collapse every summary to that level, `0` expands all.
      if (e.key === "0") app.expand_all();
      else app.collapse_to_level(Number(e.key));
      return;
    }
    // `*` opens the selected summary and every summary below it; Alt+Right below opens one level only. Shift is part of the key.
    if (e.key === "*" && !e.ctrlKey && !e.metaKey && !e.altKey && idle) {
      app.expand_selected_branch();
      return;
    }
    if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight") && idle) {
      // Alt+Left/Right navigates browser history and can discard unsaved edits.
      e.preventDefault();
      // A focused group header closes or opens its own group: it is no task, so the selection is not its target.
      const header = e.target instanceof Element ? e.target.closest<HTMLElement>('.row[data-kind="3"], .row[data-kind="4"]') : null;
      if (header !== null) {
        if ((header.dataset.kind === "3") === (e.key === "ArrowLeft")) app.toggle_group(header.dataset.group ?? null);
      } else if (e.key === "ArrowLeft") app.collapse_selected();
      else app.expand_selected();
      return;
    }
    // Indent and outdent the selected row. Alt+Shift+Left is not Alt+Left (collapse), which requires no Shift above.
    // Alt+Shift+Arrow can move a browser's selection or history: the key is taken even when the command is refused.
    if (e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey && (e.key === "ArrowLeft" || e.key === "ArrowRight") && idle) {
      e.preventDefault();
      if (e.key === "ArrowLeft") rows.outdent();
      else rows.indent();
      return;
    }
    // Move the selected row up or down among its siblings. Taken like the indent keys: Alt+Shift+Arrow can move a browser's selection.
    if (e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && idle) {
      e.preventDefault();
      if (e.key === "ArrowUp") rows.moveUp();
      else rows.moveDown();
      return;
    }
    // The cell cursor (gs#416): plain arrows move it, one visible row or one column; Home and End put it on the first and last column. With a modifier the arrows belong to
    // other keys (Alt: collapse and indent) or (Shift) to a range, below; Left and Right do nothing, and are left to the browser,
    // while nothing is selected. A cell being edited has the keys itself (the input-target return above).
    if (!e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && idle) {
      const taken =
        e.key === "ArrowDown" ? cursor.moveRows(1)
        : e.key === "ArrowUp" ? cursor.moveRows(-1)
        : e.key === "ArrowRight" ? cursor.moveColumns(1)
        : e.key === "ArrowLeft" ? cursor.moveColumns(-1)
        : e.key === "Home" ? cursor.toColumn("first")
        : e.key === "End" ? cursor.toColumn("last")
        : false;
      if (taken) {
        e.preventDefault();
        return;
      }
    }
    // Row jumps (gs#703): Ctrl+Home and Ctrl+End (Cmd+Up and Cmd+Down on a Mac) select the first and last visible row, PageUp and
    // PageDown move one page of rows. The app clamps at the ends, so a step of the whole row count reaches them.
    if (!e.altKey && !e.shiftKey && idle) {
      const mod = e.ctrlKey || e.metaKey;
      const taken =
        mod && (e.key === "Home" || (e.metaKey && e.key === "ArrowUp")) ? cursor.moveRows(-cursor.rowCount())
        : mod && (e.key === "End" || (e.metaKey && e.key === "ArrowDown")) ? cursor.moveRows(cursor.rowCount())
        : !mod && e.key === "PageUp" ? cursor.moveRows(-cursor.pageRows())
        : !mod && e.key === "PageDown" ? cursor.moveRows(cursor.pageRows())
        : false;
      if (taken) {
        e.preventDefault();
        return;
      }
    }
    // The range selection (gs#423): Shift+Up and Shift+Down move the range's other end one visible row, and Shift+Space makes the
    // selected row a range of its own row. Shift+Left and Shift+Right are left to the browser. A Shift+Space with nothing selected
    // is left to the browser too (it scrolls the page).
    if (e.shiftKey && !e.altKey && !e.ctrlKey && !e.metaKey && idle) {
      const taken =
        e.key === "ArrowDown" ? cursor.extendRows(1)
        : e.key === "ArrowUp" ? cursor.extendRows(-1)
        : e.key === " " ? cursor.selectRow()
        : false;
      if (taken) {
        e.preventDefault();
        return;
      }
    }
    // Insert and delete a row: after the input-target return above, so typing in a cell or the find box never changes a row.
    // Insert and Delete act alone; Mod+Enter and Mod+Backspace are the same two commands for a keyboard without those keys
    // (a Mac). Plain Backspace deletes nothing. Mod is Ctrl or Cmd with no Alt and no Shift, so Alt+Enter does nothing here.
    if (!e.altKey && !e.shiftKey && idle) {
      const mod = e.ctrlKey || e.metaKey;
      if ((!mod && e.key === "Insert") || (mod && e.key === "Enter")) {
        e.preventDefault();
        rows.insert();
        return;
      }
      if ((!mod && e.key === "Delete") || (mod && e.key === "Backspace")) {
        e.preventDefault();
        rows.remove();
        return;
      }
    }
    // Step through the selected task's links; Delete then removes the one selected.
    if ((e.key === "[" || e.key === "]") && !e.ctrlKey && !e.metaKey && !e.altKey && idle) {
      if (app.step_link(e.key === "]")) e.preventDefault();
      return;
    }
    // Zoom and fit (gs#703), as the Zoom group's buttons: `+` (and `=`, the same key without Shift) and `-` step about the middle
    // of the canvas, `f` fits the project. The cell cursor has taken the plain keys above while it is in the list.
    if ((e.key === "+" || e.key === "=" || e.key === "-") && !e.ctrlKey && !e.metaKey && !e.altKey && idle) {
      app.wheel(canvas.clientWidth / 2, 0, 0, e.key === "-" ? ZOOM_KEY_DY : -ZOOM_KEY_DY, true);
      e.preventDefault();
      return;
    }
    if (e.key === "f" && !e.ctrlKey && !e.metaKey && !e.altKey && idle) {
      app.fit_project();
      e.preventDefault();
      return;
    }
    if (e.key === "c" && !e.ctrlKey && !e.metaKey && !e.altKey) app.next_critical();
    if (e.key === "t" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const day = app.data_date_text().slice(0, 10);
      if (day !== "") app.go_to_date(day);
    }
    if (e.key === "C" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      app.set_critical_only(!app.critical_only());
    }
    if (e.key === "L" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      app.set_longest_path(!app.longest_path());
    }
    if (e.key === "S" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      app.set_slipped_only(!app.slipped_only());
    }
    if (e.key === "M" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      app.set_moved_only(!app.moved_only());
    }
    // The status line's key.
    if (e.key === "s" && !e.ctrlKey && !e.metaKey && !e.altKey) request("toggle-status");
    if (e.key === "Escape") {
      // A gesture in progress is cancelled first (a link drag then adds nothing on release).
      app.pointer_cancel();
      app.clear_selection();
      cursor.leave();
    }
  });
}
