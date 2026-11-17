// The task list's ID column (gs#87 PR 4): shown only when the open schedule has activity IDs. It is the chart's own work, so the
// table and the cell cursor read it here, not from the toolbar (gs#1195). It writes the column's head and `data-ids` on the
// table once at mount and again after each file opened in place, never per frame.
import { optional } from "./dom.js";
import { nameOf } from "./field-names.js";
import { type Shell } from "./shell.js";

export interface IdColumn {
  /** Read whether the open schedule has activity IDs, and show or hide the column's head and track to match. */
  show: () => void;
  /** The ID column is showing. */
  on: () => boolean;
}

export function installIdColumn({ app, root }: Shell): IdColumn {
  const table = optional<HTMLElement>(root, "table");
  const headCols = optional<HTMLElement>(root, "table-head")?.querySelector<HTMLElement>(".cols") ?? null;
  let on = false;
  const show = (): void => {
    on = table !== null && headCols !== null && app.has_codes();
    headCols?.querySelector(".code")?.remove();
    if (on) {
      const head = document.createElement("span");
      head.className = "code";
      head.textContent = nameOf(root, "id");
      headCols?.prepend(head);
      if (table !== null) table.dataset.ids = "true";
    } else if (table !== null) {
      delete table.dataset.ids;
    }
  };
  show();
  return { show, on: () => on };
}
