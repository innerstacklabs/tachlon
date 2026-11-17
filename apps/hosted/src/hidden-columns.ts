// The columns the viewer hid in the Columns menu (gantt#703): the hosted page keeps them across a reload, in `localStorage`, as
// `first-screen-line.ts` keeps its dismissal. The page does this, never `packages/gantt/`, which ships to npm and must not write a customer app's storage.
const KEY = "gantt-hidden-columns";
const COLUMNS = ["start", "finish", "float"];

const tableOf = (root: ParentNode): Element | null => root.querySelector("[data-part=table]");

function stored(): string[] {
  try {
    return (localStorage.getItem(KEY) ?? "").split(",").filter((column) => COLUMNS.includes(column));
  } catch {
    return []; // storage blocked: every column shows on each load
  }
}

/** Hide the columns kept from the last visit, on the chart's table (a switch to WebGL2 makes a new one). The menu reads the flags when it opens. */
export function restoreHiddenColumns(root: ParentNode): void {
  for (const column of stored()) tableOf(root)?.setAttribute(`data-hide-${column}`, "");
}

/** Keep the hidden columns after each press on a row of the Columns panel. The listener is on the root, so it outlives a new toolbar. */
export function keepHiddenColumns(root: HTMLElement): void {
  root.addEventListener("click", (e) => {
    const table = tableOf(root);
    if (table === null || !(e.target instanceof Element) || e.target.closest("[data-part=columns-panel] [data-column]") === null) return;
    try {
      localStorage.setItem(KEY, COLUMNS.filter((column) => table.hasAttribute(`data-hide-${column}`)).join(","));
    } catch {
      // not kept
    }
  });
}
