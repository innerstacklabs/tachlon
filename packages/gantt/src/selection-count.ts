import { optional } from "./dom.js";
import { type Status } from "./api.js";
import { say } from "./messages.js";

export interface SelectionCount {
  /** Write `5 rows selected` (in the chart's language) when the status's range changed since the last call; an idle frame compares one string and writes nothing. */
  sync: (s: Status) => void;
}

/**
 * The selection count (gs#703): with two or more rows selected, the small text after the toolbar's row count says how many.
 * The count is the table rows `range[1] - range[0] + 1`, so a collapsed summary inside the range is one row. The span is a
 * polite live region of its own (`#row-count` is the toolbar's), empty with no range, so a screen reader hears each change once.
 * The page keeps the range it last wrote as a string and writes only when that changes.
 */
export function installSelectionCount(root: ParentNode): SelectionCount {
  const span = optional<HTMLSpanElement>(root, "selection-count");
  let shown = "";
  return {
    sync: (s) => {
      if (span === null) return;
      const key = s.range === null ? "" : `${s.range[0]},${s.range[1]}`;
      if (key === shown) return;
      shown = key;
      const rows = s.range === null ? 0 : s.range[1] - s.range[0] + 1;
      span.textContent = rows < 2 ? "" : say(root, "rows-selected", { count: rows });
    },
  };
}
