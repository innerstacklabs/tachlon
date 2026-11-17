import { element } from "./dom.js";
import { nameOf } from "./field-names.js";
import { type Shell } from "./shell.js";
import { type TaskDialog } from "./task-dialog.js";

interface CellEditor {
  row: HTMLDivElement;
  cell: HTMLElement;
  input: HTMLInputElement;
  original: string;
  closing: boolean;
  error: HTMLDivElement | null;
}

/** The columns a cell edit opens on: the name, the duration, the start, the finish and, while the table shows them, four progress cells (gs#703), named by their class. */
export type EditField = "name" | "dur" | "start" | "finish" | "actual-start" | "actual-finish" | "remaining" | "percent";
/** The progress cells' field names for `App.set_progress_field`. */
const PROGRESS_FIELD: Partial<Record<EditField, string>> = { "actual-start": "actual_start", "actual-finish": "actual_finish", remaining: "remaining", percent: "percent" };
/** The key of the name a screen reader says for each editable column (`field-names.ts`). */
const FIELD_KEY: Record<EditField, string> = {
  name: "task-name",
  dur: "duration",
  start: "start",
  finish: "finish",
  "actual-start": "actual-start",
  "actual-finish": "actual-finish",
  remaining: "remaining",
  percent: "percent",
};
const EDITABLE = ["name", "dur", "start", "finish", "actual-start", "actual-finish", "remaining", "percent"] as const satisfies readonly EditField[];
const EDITABLE_SELECTOR = EDITABLE.map((field) => `.${field}`).join(", ");
const fieldOf = (cell: Element): EditField | undefined => EDITABLE.find((field) => cell.classList.contains(field));

/**
 * The editable cell of `row` under the pointer's `x`. An empty cell has no height (the row centres its cells), so a press on it
 * lands on the row; the column still tells which cell was meant.
 */
export function cellAt(row: Element, x: number): HTMLElement | null {
  for (const cell of row.children) {
    const box = cell.getBoundingClientRect();
    if (box.width > 0 && x >= box.left && x < box.right) return fieldOf(cell) === undefined ? null : (cell as HTMLElement);
  }
  return null;
}
/** Where the cursor goes after a commit: `Enter` moves down, `Tab` to the next cell, `Shift+Tab` to the previous one. */
export type EditMove = "down" | "next" | "previous";
/** One editing key of the cell cursor: a printable key's text, `Backspace`, `Enter`, `Tab`, `Shift+Tab`, or `F2` (which does nothing in an open editor). */
export type EditKey = { text: string } | "Backspace" | "Enter" | "Tab" | "Shift+Tab" | "F2";

export interface CellEdit {
  /** A cell is being edited. */
  active: () => boolean;
  /** Close the open edit, if any, and keep what the cell showed before. */
  end: () => void;
  /**
   * Open the editor on a task's name or duration cell (gs#421). With `initial` it holds that text with the caret at the end
   * (a typed key, or "" for Backspace); with null it holds the old text, selected. False, doing nothing, while another edit is
   * open or when the task's row is not drawn yet.
   */
  begin: (task: number, field: EditField, initial: string | null) => boolean;
  /**
   * Give the open editor one key, as if it were typed there: text goes in at the caret (over a selection), `Backspace` deletes,
   * `Enter` and `Tab` commit and move like the real keys. The cursor replays the keys pressed before the editor opened, in
   * order (gs#421). False, doing nothing, when no editor is open.
   */
  apply: (key: EditKey) => boolean;
}

/**
 * Double-click a name or a duration to edit it in place, any cell without an editor to open the task dialog; the cell cursor opens the same editor from the keyboard. `onEnd` runs
 * when an edit closes: the rows are drawn again. `onMove` runs after `Enter` or `Tab` committed: the cursor moves.
 */
export function installCellEdit( // oxlint-disable-line max-lines-per-function -- existing, over cap
  { app, root, status }: Shell,
  onEnd: () => void,
  onMove: (task: number, field: EditField, move: EditMove) => void,
  openDialog: TaskDialog["open"],
  refuseReadOnly: () => boolean,
): CellEdit {
  const tableRows = element<HTMLDivElement>(root, "table-rows");
  let editor: CellEditor | null = null;
  const clearEditError = (current: CellEditor): void => {
    current.input.classList.remove("invalid");
    current.input.removeAttribute("title");
    current.error?.remove();
    current.error = null;
  };
  const endEdit = (current: CellEditor): void => {
    if (current.closing || editor !== current) return;
    current.closing = true;
    clearEditError(current);
    current.cell.textContent = current.original;
    editor = null;
    onEnd();
    // The editor's input is gone: keep focus in the chart so its keys still work.
    if (!root.contains(document.activeElement) || document.activeElement === root) root.focus({ preventScroll: true });
  };
  const beginEdit = (row: HTMLDivElement, cell: HTMLElement, task: number, initial: string | null = null): void => { // oxlint-disable-line max-lines-per-function -- existing, over cap
    if (refuseReadOnly()) return;
    const field: EditField = fieldOf(cell) ?? "dur";
    const original = cell.textContent ?? "";
    const input = document.createElement("input");
    input.className = "cell-edit";
    input.dataset.field = field;
    // A date cell reads `YYYY-MM-DD HH:MM`; the editor holds the day, and a commit that leaves it alone is no edit, so an
    // imported time of day is not cut to midnight.
    const oldText = (): string =>
      field === "name" ? app.task_label(task)
      : field === "dur" ? app.task_duration(task)
      : field === "remaining" ? app.task_remaining(task)
      : field === "percent" ? app.task_percent(task)
      : field === "start" || field === "finish" ? original.slice(0, 10)
      : (field === "actual-start" ? app.task_actual_start(task) : app.task_actual_finish(task)).slice(0, 10);
    input.value = initial ?? oldText();
    // The box is named by its field and its task, so a screen reader says what is being edited.
    input.setAttribute("aria-label", `${nameOf(root, FIELD_KEY[field])}: ${app.task_label(task)}`);
    cell.style.paddingInlineStart = "0";
    // An empty progress cell has no height of its own; the editor fills the row's.
    cell.style.alignSelf = "stretch";
    cell.replaceChildren(input);
    // A Start cell holding a real constraint names it and how to clear it; an error message replaces the hint.
    const constraint = field === "start" ? app.task_constraint_text(task) : "";
    if (constraint !== "") input.title = `Start no earlier than ${constraint}. Clear the cell to remove it.`;
    const current: CellEditor = { row, cell, input, original, closing: false, error: null };
    editor = current;
    input.addEventListener("input", () => clearEditError(current));
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      // The toolbar's find box keeps its own edit history, and the browser's undo would move focus there; Escape cancels a cell edit.
      if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key.toLowerCase() === "z" || e.key.toLowerCase() === "y")) e.preventDefault();
      if (e.key === "Escape") {
        e.preventDefault();
        endEdit(current);
        return;
      }
      // Enter commits and moves down; Tab and Shift+Tab commit and move across (a Tab with Ctrl, Alt or Cmd is the browser's).
      const move: EditMove | null =
        e.key === "Enter" ? "down"
        : e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey ? (e.shiftKey ? "previous" : "next")
        : null;
      if (move === null) return;
      e.preventDefault();
      if (input.value === oldText()) {
        endEdit(current);
        onMove(task, field, move);
        return;
      }
      try {
        const progress = PROGRESS_FIELD[field];
        if (progress !== undefined) app.set_progress_field(task, progress, input.value);
        else if (field === "start") {
          // A start the links already pass is no edit: the reason shows as a refusal does, and the editor stays open.
          const note = app.set_start_text(task, input.value);
          if (note !== "") throw new Error(note);
        } else if (field === "finish") app.set_finish_text(task, input.value);
        else if (field === "name") app.rename_task(task, input.value);
        else app.set_duration(task, input.value);
        endEdit(current);
        onMove(task, field, move);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        clearEditError(current);
        input.classList.add("invalid");
        input.title = message;
        const error = document.createElement("div");
        error.className = "cell-error";
        error.setAttribute("role", "alert");
        error.textContent = message;
        row.append(error);
        current.error = error;
      }
    });
    input.addEventListener("blur", () => {
      if (!current.closing) endEdit(current);
    });
    input.focus();
    if (initial === null) input.select();
    else input.setSelectionRange(input.value.length, input.value.length);
  };
  tableRows.addEventListener("dblclick", (e) => {
    if (!(e.target instanceof Element)) return;
    if (e.target.closest(".disclosure") !== null) return;
    const row = e.target.closest<HTMLDivElement>(".row");
    const cell = e.target.closest<HTMLElement>(EDITABLE_SELECTOR) ?? (row === null ? null : cellAt(row, e.clientX));
    if (editor !== null && cell !== editor.cell) endEdit(editor);
    const task = Number(row?.dataset.task);
    if (row === null || !Number.isInteger(task)) return;
    // A cell with no editor of its own (the ID cell, a float cell) opens the task dialog instead (gantt#1875).
    if (cell === null) {
      openDialog(task);
      return;
    }
    if (editor !== null || status().gesture !== "idle") return;
    beginEdit(row, cell, task);
  });
  return {
    active: () => editor !== null,
    end: () => {
      if (editor !== null) endEdit(editor);
    },
    begin: (task, field, initial) => {
      if (editor !== null || status().gesture !== "idle") return false;
      const row = tableRows.querySelector<HTMLDivElement>(`.row[data-task="${task}"]`);
      const cell = row?.querySelector<HTMLElement>(`:scope > .${field}`);
      if (!row || !cell) return false;
      beginEdit(row, cell, task, initial);
      return true;
    },
    apply: (key) => {
      if (editor === null || editor.closing) return false;
      const { input } = editor;
      if (key === "F2") return true;
      if (key === "Enter" || key === "Tab" || key === "Shift+Tab") {
        input.dispatchEvent(new KeyboardEvent("keydown", { key: key === "Enter" ? "Enter" : "Tab", shiftKey: key === "Shift+Tab", bubbles: true, cancelable: true }));
        return true;
      }
      const start = input.selectionStart ?? input.value.length;
      const end = input.selectionEnd ?? start;
      if (key !== "Backspace") input.setRangeText(key.text, start, end, "end");
      else if (start !== end) input.setRangeText("", start, end, "end");
      else if (start > 0) input.setRangeText("", start - 1, start, "end");
      input.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    },
  };
}
