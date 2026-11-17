import { type Status } from "./api.js";
import { element } from "./dom.js";

export interface RangeMarks {
  /** Tint the table rows of the range selection; `redrawn` says the rows were replaced. Called with each frame's status. */
  mark: (s: Status, redrawn: boolean) => void;
}

/**
 * The range selection's tint (gs#423, story S2 of #300). The app owns the range and reports it as table rows in
 * `status().range`; this file only marks the rows that are in the DOM, which are the drawn window, so a range of
 * thousands of rows costs the same as one of ten. A row is marked `data-range="true"` and the stylesheet draws it.
 */
export function installRangeMarks(root: ParentNode): RangeMarks {
  const tableRows = element<HTMLDivElement>(root, "table-rows");
  let marked = "";
  return {
    mark: (s, redrawn) => {
      const key = s.range === null ? "" : `${s.range[0]}:${s.range[1]}`;
      // Rows are replaced on a redraw, so the marks are applied again then; otherwise only when the range changes.
      if (!redrawn && key === marked) return;
      marked = key;
      for (const row of tableRows.children) {
        if (!(row instanceof HTMLElement)) continue;
        const at = Number(row.dataset.row);
        if (s.range !== null && at >= s.range[0] && at <= s.range[1]) row.dataset.range = "true";
        else delete row.dataset.range;
      }
    },
  };
}
