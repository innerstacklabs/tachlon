import { isHidden } from "./columns-menu.js";
import { nameOf } from "./field-names.js";

/**
 * Put the Free float head in step with the Columns menu: the head cell and the row cells exist only while the column shows (it opens hidden, and a
 * hidden column leaves the table as it was, as the Predecessors column does). Returns whether the column shows. Asked when the rows are drawn,
 * never per frame of the chart.
 */
export function syncFreeFloatHead(root: object, table: Element, headCols: Element | null): boolean {
  const shown = !isHidden(table, "free-float");
  const head = headCols?.querySelector(".free-float") ?? null;
  if (shown === (head !== null)) return shown;
  head?.remove();
  if (shown) headCols?.querySelector(".float")?.after(Object.assign(document.createElement("span"), { className: "free-float", textContent: nameOf(root, "free-float-head"), title: nameOf(root, "free-float-title") }));
  return shown;
}
