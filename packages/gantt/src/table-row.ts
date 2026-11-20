import { type HeaderRow, type PublicApp, type TableRow } from "./api.js";
import { type Say } from "./messages.js";
import { CHEVRON_PX, INDENT_PX } from "./task-width.js";

/** A variance cell's text: Rust writes "0" for no change, which the table shows as a middot; a row with no variance is empty. */
const varText = (text: string | null): string => (text === "0" ? "\u00b7" : (text ?? ""));

/** A Start cell holding a real start constraint (`constraint` is its date, not empty; never the floor an import gave) carries the mark and names the date (gs#703 K7). */
function markConstraint(cell: HTMLElement, constraint: string, say: Say): void {
  if (constraint === "") return;
  cell.dataset["constrained"] = "true";
  cell.title = say("start-no-earlier-hint", { date: constraint });
}

/** What a row's cells depend on, fixed for one redraw of the window. */
export interface RowContext {
  app: PublicApp;
  say: Say;
  rowHeight: number;
  hasCodes: () => boolean;
  hasWbs: boolean;
  hasPreds: boolean;
  hasResources: boolean;
  holdResources: boolean;
  hasProgress: boolean;
  /** The Free float column shows (the Columns menu has not hidden it): the row has its cell. A hidden column has no cell and no head, so a table keeps the cells it had. */
  freeFloat: boolean;
  /** A bar is being dragged: one empty cell stands in for the three variance cells. */
  holdVariance: boolean;
  withVariance: boolean;
  /** The task slots that wait: the ones the host marked for its own recalculation (`Gantt.setPending`) and the ones whose commit the host's `save` has not saved. */
  pending: ReadonlySet<number>;
  /** The line a refused change shows at the cell it names (decision 0022, rule 3.5). */
  refusal: Refusal | null;
}

/** A refused change's message, for the task `task` and the cell `cell` (its class). */
export interface Refusal { task: number; cell: string; message: string }

/** The message of a refusal, as a note under the row; `placeRefusal` moves it under its cell once the row is on the page. */
function refusalNote({ cell, message }: Refusal): HTMLDivElement {
  const note = document.createElement("div");
  note.className = "cell-error";
  note.setAttribute("role", "alert");
  note.dataset["cell"] = cell;
  note.textContent = message;
  return note;
}

/** Start the refusal's note at its cell, as a cell edit's reason starts (cell-edit.ts); a cell the table does not show leaves it at the row's start. */
export function placeRefusal(rows: HTMLElement): void {
  const note = rows.querySelector<HTMLElement>(".cell-error[data-cell]");
  const cell = note?.parentElement?.querySelector<HTMLElement>(`.${note.dataset["cell"]}`);
  if (note !== undefined && note !== null && cell !== null && cell !== undefined) Object.assign(note.style, { insetInlineStart: "auto", left: `${cell.offsetLeft}px` });
}

/** A row the host marked pending (`Gantt.setPending`): `data-pending` dims its cells, and `aria-busy` tells a screen reader it is busy. */
function markPending(row: HTMLElement): void {
  row.dataset["pending"] = "true";
  row.setAttribute("aria-busy", "true");
}

/** A group header's row (decision 0015): the value and its leaf count in one cell over the columns, with the chevron that opens and closes the group. It has no task, so no `data-task`. */
export function buildHeader([, label, count, , , kind]: HeaderRow, r: number, ctx: RowContext): HTMLDivElement {
  const row = document.createElement("div");
  row.className = `row cols${r % 2 === 1 ? " odd" : ""}`;
  row.style.top = `${r * ctx.rowHeight}px`;
  row.dataset.row = String(r);
  row.dataset.kind = String(kind);
  if (label !== null) row.dataset.group = label;
  // The leaves with no value have no text to show: an em dash stands for it, and a real value spelled "(none)" is its own group.
  const text = `${label ?? "—"} (${count})`;
  const cell = document.createElement("span");
  cell.className = "name";
  cell.style.gridColumn = "2 / -1";
  cell.textContent = text;
  cell.title = label ?? "No value";
  const disclosure = document.createElement("button");
  disclosure.type = "button";
  disclosure.className = "disclosure";
  disclosure.setAttribute("aria-expanded", String(kind === 3));
  disclosure.setAttribute("aria-label", ctx.say(kind === 3 ? "collapse" : "expand", { name: text }));
  cell.prepend(disclosure);
  row.append(cell);
  return row;
}

/** One table row's element, for row `r` of the schedule, or a group header's (kind 3 or 4); the cells follow the columns the context says are shown. */
export function buildRow(data: TableRow | HeaderRow, r: number, ctx: RowContext): HTMLDivElement {
  if (data[5] === 3 || data[5] === 4) return buildHeader(data as HeaderRow, r, ctx);
  const [task, start, finish, float, critical, kind, ...variance] = data as TableRow;
  const { app, hasCodes, hasWbs, hasPreds, hasResources, holdResources, hasProgress, holdVariance, withVariance } = ctx;
  const row = document.createElement("div");
  row.className = `row cols${r % 2 === 1 ? " odd" : ""}${critical ? " critical" : ""}`;
  row.style.top = `${r * ctx.rowHeight}px`;
  Object.assign(row.dataset, { row: String(r), task: String(task), kind: String(kind) });
  if (ctx.pending.has(task)) markPending(row);
  // Ledger's column order; the ID column comes first, for a file that has activity IDs, then the WBS column, for a file that has WBS codes. --gantt-cols sizes them.
  const cells: [string, string][] = [
    ["name", app.task_label(task)],
    ["dur", app.task_duration(task)],
    ["start", start],
    ["finish", finish],
    ["float", float === null ? "" : app.task_float(task)], ...(ctx.freeFloat ? [["free-float", variance[3] ?? ""] as [string, string]] : []), ...(hasPreds ? [["pred", kind === 0 ? (app.predecessors_text(task) ?? "") : ""] as [string, string]] : []),
  ];
  // The variance columns come while a baseline is set (data-variance, set by the toolbar) and no bar is being dragged: a drag
  // redraws every row every frame, and three more cells on each cost the frame more than the columns are worth while the
  // numbers are moving. In their place a drag puts one empty cell that spans the three tracks, so the cells after them (the
  // progress cells) stay under their heads, and the cells come back at the release. A zero is a muted middot, so only a
  // change stands out; a later finish and less float read in the critical colour.
  if (holdVariance) cells.push(["var-hold", ""]);
  if (withVariance) {
    const [varStart, varFinish, varFloat] = variance;
    const zero = (text: string | null): string => (text === "0" ? " zero" : "");
    cells.push(
      ["var-start" + zero(varStart), varText(varStart)],
      ["var-finish" + zero(varFinish) + (varFinish?.startsWith("+") ? " late" : ""), varText(varFinish)],
      ["var-float" + zero(varFloat) + (varFloat?.startsWith("-") || varFloat?.startsWith(">-") ? " lost" : ""), varText(varFloat)],
    );
  }
  if (hasProgress) {
    cells.push(
      ["actual-start", app.task_actual_start(task)],
      ["actual-finish", app.task_actual_finish(task)],
      ["remaining", app.task_remaining(task)],
      ["percent", app.task_percent(task)],
    );
  }
  if (hasResources) cells.push(["resources", holdResources || kind !== 0 ? "" : app.task_resources_text(task)]);
  if (hasWbs) cells.unshift(["wbs", app.task_wbs_code(task)]);
  if (hasCodes()) cells.unshift(["code", app.task_code(task)]);
  for (const [cls, text] of cells) {
    const cell = document.createElement("span");
    cell.className = cls;
    cell.textContent = text;
    if (cls === "wbs" || cls === "resources" || cls === "pred") cell.title = text;
    if (cls === "start" && kind === 0) markConstraint(cell, app.task_constraint_text(task), ctx.say);
    if (cls === "name") {
      // The row shows the leaf name; the outline path is the tooltip and the hover card's.
      // A leaf leaves room for the chevron its summary siblings carry, so sibling names line up.
      cell.style.paddingInlineStart = `${(app.task_outline_level(task) - 1) * INDENT_PX + (kind === 0 ? CHEVRON_PX : 0)}px`;
      cell.title = app.task_name(task);
      if (kind !== 0) {
        const expanded = kind === 1;
        const disclosure = document.createElement("button");
        disclosure.type = "button";
        disclosure.className = "disclosure";
        disclosure.setAttribute("aria-expanded", String(expanded));
        disclosure.setAttribute("aria-label", ctx.say(expanded ? "collapse" : "expand", { name: text }));
        cell.prepend(disclosure);
      }
    }
    row.append(cell);
  }
  return ctx.refusal?.task === task ? (row.append(refusalNote(ctx.refusal)), row) : row;
}
