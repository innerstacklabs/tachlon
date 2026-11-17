import { optional } from "./dom.ts"; // `.ts`, so the Node tier can load this file
import { say } from "./messages.ts";

/** How long the label stays up after the last scroll, in milliseconds. */
export const POSITION_MS = 1000;

export interface RowPosition {
  /** Write `Row 501 of 2,000` for the row at the top of the list and show it; it hides one second after the last call. */
  show: (scrollTop: number, rowHeight: number, rowCount: number) => void;
}

/**
 * The row position (gs#703): at the list's right edge, "Row N of M" for one second after the list scrolls, hidden at rest.
 * The table's scroll handler calls `show`, so the label is a DOM write per scroll event and no work in the frame loop.
 * N is the row at the top of the list; the text is written only when it changes. `fmt` is the chart's number format, so it follows the page's `locale`.
 */
export function installRowPosition(root: ParentNode, fmt: Intl.NumberFormat): RowPosition {
  const label = optional<HTMLSpanElement>(root, "row-position");
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    show: (scrollTop, rowHeight, rowCount) => {
      if (label === null || rowCount < 1) return;
      const row = Math.min(rowCount, Math.floor(scrollTop / rowHeight) + 1);
      const text = say(root, "row-position", { row: fmt.format(row), total: fmt.format(rowCount) });
      if (label.textContent !== text) label.textContent = text;
      label.hidden = false;
      clearTimeout(timer);
      timer = setTimeout(() => {
        label.hidden = true;
      }, POSITION_MS);
    },
  };
}
