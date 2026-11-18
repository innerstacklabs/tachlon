import { type Status } from "./api.js";
import { arranged } from "./column-order.js";
import { isHidden } from "./columns-menu.js";
import { type CellEdit, type EditField, type EditKey, type EditMove, cellAt } from "./cell-edit.js";
import { element } from "./dom.js";
import { drawn, modeOf, type Mode } from "./mode.js";
import { installRangeMarks } from "./range.js";
import { listening, type Shell } from "./shell.js";

/** The table's columns, left to right; `code` is there only while the table shows the ID column, `wbs` only while it shows the WBS column, `pred` only while it shows the Predecessors column, the four progress cells (gs#703) only while it shows the progress columns. */
const COLUMNS = ["code", "wbs", "name", "dur", "start", "finish", "float", "pred", "actual-start", "actual-finish", "remaining", "percent"] as const;
const PROGRESS: readonly string[] = ["actual-start", "actual-finish", "remaining", "percent"];
/** The cells that take an edit, in the order Tab walks a run of them: the name and duration, then the progress cells. */
const RUNS: readonly (readonly EditField[])[] = [["name", "dur"], ["finish", "pred"], ["actual-start", "actual-finish", "remaining", "percent"]];
const editable = (name: Column): name is EditField => RUNS.some((run) => (run as readonly string[]).includes(name));
type Column = (typeof COLUMNS)[number];

export interface Cursor {
  /** Arrow Up (-1) and Down (1): move the selection that many visible rows. With nothing selected it selects the first row. True when the app moved it. */
  moveRows: (delta: number) => boolean;
  /** The rows that fit in the table body (at least one): the step of PageUp and PageDown. */
  pageRows: () => number;
  /** The visible task rows: the step that carries Ctrl+Home and Ctrl+End to the first and last row. */
  rowCount: () => number;
  /** Arrow Left (-1) and Right (1): move the cursor to the next column. False, doing nothing, with no cursor. */
  moveColumns: (delta: number) => boolean;
  /** Home and End: put the cursor on the first or last column the table has for this file; the row stays. False, doing nothing, with no cursor. */
  toColumn: (end: "first" | "last") => boolean;
  /** Put the cursor on the selected row's name cell, in the grid: what Insert does, so the next keys name the new row. */
  toName: () => void;
  /** Shift+Up (-1) and Shift+Down (1): move the range's other end that many visible rows (gs#423). True when the app moved it. */
  extendRows: (delta: number) => boolean;
  /** Shift+Space: make the selected row a range of its own row. False, doing nothing, with nothing selected. */
  selectRow: () => boolean;
  /** Ctrl+A: make every visible row a range, the first row its anchor. False, doing nothing, while a cell is edited or a bar is dragged, or with no rows. */
  selectAll: () => boolean;
  /** Draw the cursor on the selected row's cell and tint the range's rows; `redrawn` says the rows were replaced. Called with each frame's status. */
  mark: (s: Status, redrawn: boolean) => void;
  /**
   * The cursor's own keys while it is in the grid (gs#421): `Enter` and `F2` edit the cell with its old text, `Backspace` edits
   * it empty, a printable key edits it holding that key. Every printable key is taken, even on a cell that cannot be edited, so
   * the chart's letter keys are suspended. Keys pressed before the row is drawn wait with it, in order (`Backspace`, `Enter`
   * and `Tab` too), and are replayed when the editor opens. True when the key was taken.
   */
  key: (e: KeyboardEvent) => boolean;
  /** Where the keyboard is: the grid only while the cursor was entered and the selected row is drawn (mode.ts). */
  mode: () => Mode;
  /** Escape: the cursor leaves the grid and the chart's letter keys return. */
  leave: () => void;
  /** An edit of `task`'s `field` was committed with `move`: put the cursor where the move says. */
  moved: (task: number, field: EditField, move: EditMove) => void;
}

/**
 * The cell cursor (gs#416, story S1 of #300). Its row is the selected task: the app owns it, keeps it across edits and
 * undo, and moves it over the visible rows, so this file never counts rows. Its column is presentation, kept here as a
 * name (not an index, because the ID column comes and goes). A click on a cell selects its row and sets the column;
 * Escape clears the selection, and the column resets with it.
 *
 * The cursor is "in the grid" (gs#421) from a click on a cell or an arrow until Escape, a press on the timeline, focus leaving the
 * chart, or the end of the selection or its row being hidden (a collapsed summary, a filter); only then does it take the keys that
 * edit a cell and suspend the chart's letter keys (`mode.ts`).
 */
export function installCursor(shell: Shell, edit: CellEdit, hasCodes: () => boolean, linking: () => boolean): Cursor { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { app, root, status, canvas } = shell;
  const tableRows = element<HTMLDivElement>(root, "table-rows");
  const rangeMarks = installRangeMarks(root);
  let column: Column = "name";
  let marked: { cell: HTMLElement; key: string } | null = null;
  let entered = false;
  // Keys typed before the selected row was drawn (the table redraws on the next frame), in the order typed: the first opens the
  // editor when the row is there, the rest are replayed into it.
  let pending: { task: number; keys: EditKey[]; at: number } | null = null;
  const mode = (): Mode =>
    modeOf({ entered, selected: status().selected, rowCount: status().rowCount, cellEditing: edit.active(), linkEditing: linking() });
  const leave = (): void => {
    entered = false;
    pending = null;
  };
  canvas.addEventListener("pointerdown", leave);
  // Focus that goes outside the chart takes the cursor out of the grid. A focusout with no target (the window blurred, or an
  // editor's input was removed) leaves it if focus is then outside the root: an ended edit puts focus back on the root first.
  listening(shell, root, "focusout", (e: FocusEvent) => {
    if (e.relatedTarget instanceof Node && !root.contains(e.relatedTarget)) leave();
    else if (e.relatedTarget === null) setTimeout(() => root.contains(document.activeElement) || leave(), 0);
  });
  /** The initial text a key opens the editor with; undefined for a key that cannot open it (`Tab`). */
  const opening = (key: EditKey): string | null | undefined =>
    typeof key === "object" ? key.text : key === "Backspace" ? "" : key === "Enter" || key === "F2" ? null : undefined;
  const editKey = (e: KeyboardEvent): EditKey | undefined =>
    // Shift+Space is the range's (gs#423): it makes the selected row a range, and opens no editor.
    e.key === " " && e.shiftKey ? undefined
    : e.key.length === 1 ? { text: e.key }
    : e.key === "Tab" ? (e.shiftKey ? "Shift+Tab" : "Tab")
    : e.shiftKey ? undefined
    : e.key === "Backspace" || e.key === "Enter" || e.key === "F2" ? e.key
    : undefined;
  /** Give the open editor the keys that waited for it. A commit that closes it moves the cursor: the keys left wait for that row. */
  const replay = (keys: EditKey[]): void => {
    for (let i = 0; i < keys.length; i++) {
      if (edit.apply(keys[i])) continue;
      // A commit consumed only its own key. What follows goes to the cell the cursor moved to, in order, Tabs included, from the
      // next key that can open an editor; a Tab before it has no editor to commit (live, it would be the browser's) and is dropped.
      const at = keys.findIndex((key, j) => j >= i && opening(key) !== undefined);
      const task = status().selected?.task;
      if (at >= 0 && task !== undefined) pending = { task, keys: keys.slice(at), at: performance.now() };
      return;
    }
  };
  const columns = (): readonly Column[] => {
    const table = tableRows.closest<HTMLElement>("[data-part=table]");
    const dataset = table?.dataset;
    // A column the Columns menu hid has no cell on screen, so the cursor skips it.
    return arranged(COLUMNS, table).filter(
      (name) =>
        (name !== "code" || hasCodes()) && (name !== "wbs" || dataset?.wbs === "true") && (name !== "pred" || dataset?.pred !== undefined) && (dataset?.progress === "true" || !PROGRESS.includes(name)) && !(table && isHidden(table, name)),
    );
  };
  // A Shift+click extends the range, so the browser must not also select the text between the two clicks.
  tableRows.addEventListener("mousedown", (e) => {
    if (!e.shiftKey || edit.active() || !(e.target instanceof Element) || e.target.closest(".row") === null) return;
    e.preventDefault();
    root.focus({ preventScroll: true });
  });
  tableRows.addEventListener("click", (e) => {
    const target = e.target;
    if (e.button !== 0 || edit.active() || !(target instanceof Element)) return;
    // The chevron has its own handler; it toggles the summary and selects nothing.
    if (target.closest(".disclosure") !== null) return;
    const row = target.closest<HTMLElement>(".row");
    const task = Number(row?.dataset.task);
    if (row === null || !Number.isInteger(task)) return;
    // Shift+click extends the range from the selected row to this one; the cursor's column stays.
    if (e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
      app.extend_to(task);
      return;
    }
    // An empty cell has no height, so a press on it lands on the row: the pointer's x picks the cell.
    const under = cellAt(row, e.clientX);
    const clicked = columns().find((name) => (under ?? target).closest(`.row > .${name}`) !== null);
    if (!app.select_task(task)) return;
    if (clicked !== undefined) column = clicked;
    entered = true;
  });
  const tableBody = element<HTMLDivElement>(root, "table-body");
  return {
    pageRows: () => Math.max(1, Math.floor(tableBody.clientHeight / status().rowHeight)),
    rowCount: () => status().rowCount,
    moveRows: (delta) => {
      if (edit.active() || status().gesture !== "idle") return false;
      pending = null;
      const moved = app.move_selection(delta) !== undefined;
      if (moved) entered = true;
      return moved;
    },
    extendRows: (delta) => {
      if (edit.active() || status().gesture !== "idle") return false;
      return app.extend_selection(delta) !== undefined;
    },
    selectRow: () => {
      if (edit.active() || status().gesture !== "idle") return false;
      return app.select_row_range();
    },
    selectAll: () => {
      if (edit.active() || status().gesture !== "idle") return false;
      const taken = app.select_all_rows();
      if (taken) entered = true;
      return taken;
    },
    moveColumns: (delta) => {
      if (!drawn(status().selected, status().rowCount)) return false;
      const names = columns();
      const at = Math.max(0, names.indexOf(column));
      column = names[Math.min(names.length - 1, Math.max(0, at + delta))];
      pending = null;
      entered = true;
      return true;
    },
    toName: () => {
      column = "name";
      entered = true;
    },
    toColumn: (end) => {
      if (!drawn(status().selected, status().rowCount)) return false;
      const names = columns();
      column = end === "first" ? names[0] : names[names.length - 1];
      pending = null;
      entered = true;
      return true;
    },
    key: (e) => {
      if (e.ctrlKey || e.metaKey || e.altKey || mode() !== "grid") return false;
      // A focused button, link or box keeps its own Enter and Space; every other key still reaches the cursor (gs#480).
      if ((e.key === "Enter" || e.key === " ") && e.target instanceof Element && e.target.closest("button, a, select, textarea, [contenteditable]") !== null) return false;
      const typed = editKey(e);
      const task = status().selected?.task;
      if (typed === undefined || task === undefined) return false;
      const queued = pending !== null && pending.task === task;
      // Only a key that opens the editor starts an edit; Tab and Shift+Tab are the browser's until one is waiting for them.
      if (opening(typed) === undefined && !queued) return false;
      // The progress columns went (the Progress button): the cursor is back on the name.
      if (!columns().includes(column)) column = "name";
      if (!editable(column)) return typeof typed === "object";
      if (queued) {
        pending?.keys.push(typed);
        return true;
      }
      if (!edit.begin(task, column, opening(typed) ?? null)) pending = { task, keys: [typed], at: performance.now() };
      return true;
    },
    mode,
    leave,
    moved: (task, field, move) => {
      if (status().selected?.task !== task) return;
      column = field;
      entered = true;
      if (move === "down") {
        app.move_selection(1);
        return;
      }
      // A file with no IDs has no Predecessors cell for Tab to reach.
      const run = (RUNS.find((names) => names.includes(field)) ?? RUNS[0]).filter((name) => name === field || columns().includes(name));
      const at = run.indexOf(field) + (move === "next" ? 1 : -1);
      if (at >= 0 && at < run.length) column = run[at];
      else if (app.move_selection(move === "next" ? 1 : -1) !== undefined) column = move === "next" ? run[0] : run[run.length - 1];
    },
    mark: (s, redrawn) => {
      rangeMarks.mark(s, redrawn);
      if (pending !== null) {
        const sel = s.selected?.task;
        if (sel !== pending.task || performance.now() - pending.at > 1000) pending = null;
        else if (!edit.active() && editable(column) && edit.begin(sel, column, opening(pending.keys[0]) ?? null)) {
          const rest = pending.keys.slice(1);
          pending = null;
          replay(rest);
        }
      }
      // The selection ended (Escape, another schedule opened, the row deleted): the cursor leaves the grid with it.
      if (s.selected === null) {
        column = "name";
        entered = false;
      }
      const key = s.selected === null ? "" : `${s.selected.row}:${column}`;
      if (!redrawn && key === (marked?.key ?? "") && marked?.cell.isConnected !== false) return;
      if (marked) delete marked.cell.dataset.cursor;
      marked = null;
      if (s.selected === null) return;
      const row = tableRows.querySelector<HTMLElement>(`.row[data-row="${s.selected.row}"]`);
      // The ID column is gone while no task has an ID: the cursor falls back to the name.
      const cell = row?.querySelector<HTMLElement>(`:scope > .${column}`) ?? row?.querySelector<HTMLElement>(":scope > .name");
      if (!cell) return;
      cell.dataset.cursor = "true";
      marked = { cell, key };
    },
  };
}
