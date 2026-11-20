import { type Shell, listening } from "./shell.js";
import type { PageSize, PrintPages } from "./api.js";
import { columnOrder, MOVABLE, setColumnOrder, type Movable } from "./column-order.js";
import { HIDEABLE, isHidden } from "./columns-menu.js";
import { setProgress } from "./progress-toggle.js";
import { setVariance } from "./variance-toggle.js";

/** Today's date in local time as `YYYY-MM-DD` (`toISOString` is UTC and can name the wrong day). The app keeps no clock, so the page passes this to `App.print_pages`. */
export function localDate(now: Date = new Date()): string {
  const two = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}`;
}

let canvas: CanvasRenderingContext2D | undefined;
/** The width in px of `text` set in `size` px sans-serif, as the page's own text is drawn: `App.print_pages` cuts a name only when this says it overflows its column. */
export function measureText(text: string, size: number): number {
  canvas ??= document.createElement("canvas").getContext("2d")!; // no 2D canvas throws here, and `App.print_pages` reports it
  canvas.font = `${size}px sans-serif`;
  return canvas.measureText(text).width;
}

const HEADS: Record<Movable, string> = { dur: "Dur", start: "Start", finish: "Finish", float: "Float", "free-float": "Free float" };
const PROGRESS_HEADS = ["% complete", "Actual start", "Actual finish", "Remaining"];
const VARIANCE_HEADS = ["Start variance", "Finish variance", "Float variance"];

/**
 * The list columns the screen shows, by the names `App.print_pages` takes, in the screen's order: ID, WBS and Task, the four progress columns while the
 * Progress button has them on, then Dur, Start, Finish and Float in the table's order less those the Columns menu hid, then the three variance columns
 * unless the Variance button hid them. The app leaves out a column the file has none of (no IDs, no baseline, no progress). `undefined` without a table:
 * every column prints.
 */
export function printColumns(root: ParentNode): string[] | undefined {
  const table = root.querySelector<HTMLElement>("[data-part=table]");
  if (!table) return undefined;
  const moved = columnOrder(table).filter((column) => !isHidden(table, column));
  const progress = table.dataset.progress === "true" ? PROGRESS_HEADS : [];
  const variance = table.dataset.varianceHidden === undefined ? VARIANCE_HEADS : [];
  return ["ID", "WBS", "Task", ...progress, ...moved.map((column) => HEADS[column]), ...variance];
}

/**
 * Put the screen's columns as a layout's `columns` names them, the inverse of `printColumns` (gantt#703): Start, Finish, Float and Free float that the list leaves out are hidden with the Columns menu's flags,
 * the five movable columns take the list's order (any it leaves out keep their places after it), and the Progress and Variance flags are set to match whether any column of their group is named (`setProgress` and `setVariance`, the buttons' own path, so it works without the toolbar).
 * `null` is every column: nothing hidden, Progress and Variance on, the order as it is. Without a table it does nothing.
 */
export function applyColumns(root: ParentNode & EventTarget, columns: readonly string[] | null): void {
  const table = root.querySelector<HTMLElement>("[data-part=table]");
  if (!table) return;
  // `null` names every column a layout could name before Free float existed, so it hides Free float.
  const shown = (name: string): boolean => (columns === null ? name !== HEADS["free-float"] : columns.includes(name));
  for (const column of HIDEABLE) table.toggleAttribute(`data-hide-${column}`, !shown(HEADS[column]));
  const listed = (columns ?? []).flatMap((name) => MOVABLE.filter((column) => HEADS[column] === name));
  setColumnOrder(table, [...new Set([...listed, ...columnOrder(table)])]);
  setProgress(root, PROGRESS_HEADS.some(shown));
  setVariance(root, VARIANCE_HEADS.some(shown));
}

/**
 * Ctrl+P (gs#703): before the browser prints, `.gantt-print-pages` takes one `svg` per page from `App.print_pages(title, date, size)`, and the print sheet takes the visible rows as text; `print.css` shows the pages in place of everything else. Both are emptied afterwards. This runs on `beforeprint`, never per frame.
 * Returns `printOn(size)`, which prints once on that paper: A4 adds an `@page { size: A4 landscape }` rule that `afterprint` removes, so the next print (Ctrl+P too) is Letter again.
 */
export function installPrintSheet(shell: Shell): (size: PageSize) => void {
  const { root, app } = shell;
  const sheet = root.querySelector<HTMLElement>("[data-part=print-sheet]");
  const pages = root.querySelector<HTMLElement>(".gantt-print-pages");
  let size: PageSize = "letter";
  let rule: HTMLStyleElement | undefined;
  const undo = (): void => {
    size = "letter";
    rule?.remove();
    rule = undefined;
  };
  const printOn = (to: PageSize): void => {
    undo();
    size = to;
    if (to === "a4") {
      rule = document.head.appendChild(document.createElement("style"));
      rule.textContent = "@page { size: A4 landscape; }"; // the page's own margin rule still applies
    }
    window.print();
  };
  if (!sheet) return printOn;
  listening(shell, window, "beforeprint", () => {
    sheet.dataset.title = document.title;
    sheet.textContent = app.list_text();
    if (pages) pages.innerHTML = (app.print_pages(document.title, localDate(), size, measureText, printColumns(root)) as PrintPages).join("");
  });
  listening(shell, window, "afterprint", () => {
    sheet.textContent = "";
    delete sheet.dataset.title;
    pages?.replaceChildren();
    undo();
  });
  return printOn;
}
