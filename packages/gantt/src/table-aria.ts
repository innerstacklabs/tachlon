import { type Status } from "./api.js";
import { element } from "./dom.js";
import { type Shell, watching } from "./shell.js";

/** What the app keeps of the table's ARIA tree grid: `sync` follows the row count; the observers do the rest. */
export interface TableAria {
  sync(s: Status): void;
}

/**
 * The task list is a tree grid for assistive technology (gs#703): `#table` carries the treegrid role in the page, the head is a row of
 * column headers and each drawn row a row of grid cells with its index in the whole list (the head counting as row 1), its outline
 * depth as `aria-level` and, on a summary, `aria-expanded` (a collapse draws new rows, so the pass runs again). The table
 * rebuilds its rows as the window moves and marks the selected row and the range with `data-selected` and `data-range`; this
 * module only watches those writes, so the table's own code is unchanged and sighted users see nothing different.
 */
export function installTableAria(shell: Shell, s0: Status): TableAria {
  const { root, app } = shell;
  const grid = element<HTMLElement>(root, "table");
  const head = element<HTMLElement>(root, "table-head");
  const headRow = head.querySelector<HTMLElement>(".cols");
  const body = element<HTMLElement>(root, "table-body");
  const rows = element<HTMLElement>(root, "table-rows");
  // The wrappers between the grid and its rows own no role of their own.
  head.setAttribute("role", "rowgroup");
  body.setAttribute("role", "presentation");
  rows.setAttribute("role", "rowgroup");

  const selected = (row: HTMLElement): void => {
    if (row.dataset.selected === "true" || row.dataset.range === "true") row.setAttribute("aria-selected", "true");
    else row.removeAttribute("aria-selected");
  };
  const roleRow = (row: HTMLElement): void => {
    row.setAttribute("role", "row");
    // Row 0 of the list is the second row of the grid.
    row.setAttribute("aria-rowindex", String(Number(row.dataset.row) + 2));
    // A group header has no task, so no level.
    if (row.dataset.task !== undefined) row.setAttribute("aria-level", String(app.task_outline_level(Number(row.dataset.task))));
    // data-kind: 0 a leaf, 1 an open summary, 2 a collapsed one, 3 an open group header, 4 a closed one.
    if (row.dataset.kind === "0") row.removeAttribute("aria-expanded");
    else row.setAttribute("aria-expanded", String(row.dataset.kind === "1" || row.dataset.kind === "3"));
    for (const cell of row.children) cell.setAttribute("role", "gridcell");
    selected(row);
  };
  watching(shell, new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes) if (node instanceof HTMLElement && node.dataset.row !== undefined) roleRow(node);
    }
  })).observe(rows, { childList: true });
  for (const row of rows.querySelectorAll<HTMLElement>(":scope > .row[data-row]")) roleRow(row);
  watching(shell, new MutationObserver((records) => {
    for (const record of records) if (record.target instanceof HTMLElement) selected(record.target);
  })).observe(rows, { subtree: true, attributes: true, attributeFilter: ["data-selected", "data-range"] });

  // The head's columns come and go with the file (ID, WBS, progress): one pass each time its children change.
  const roleHead = (): void => {
    if (headRow === null) return;
    headRow.setAttribute("role", "row");
    headRow.setAttribute("aria-rowindex", "1");
    for (const cell of headRow.children) cell.setAttribute("role", "columnheader");
    grid.setAttribute("aria-colcount", String(headRow.children.length));
  };
  if (headRow !== null) watching(shell, new MutationObserver(roleHead)).observe(headRow, { childList: true });
  roleHead();

  let rowCount = -1;
  const sync = (s: Status): void => {
    if (s.rowCount === rowCount) return;
    rowCount = s.rowCount;
    grid.setAttribute("aria-rowcount", String(rowCount + 1));
  };
  sync(s0);
  return { sync };
}
