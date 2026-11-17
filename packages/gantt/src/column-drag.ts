import { columnOrder, dropped, MOVABLE, setColumnOrder, type Movable } from "./column-order.js";
import { optional } from "./dom.js";
import type { Shell } from "./shell.js";

/** How far (px) the pointer must travel from the press before it is a drag and not a click. */
const SLOP = 4;

/**
 * Drag the head cell of Dur, Start, Finish or Float onto another of the four: the column takes the other's place, in the head and every row,
 * as the Columns menu's move buttons do (column-order.ts, `dropped`). The head holds the pointer from the press, so the moves and the release
 * reach it wherever they fall. While the pointer is over a target it carries `data-drop="before"` or `"after"`, the side the column will
 * land on. A release on Task, off the head, or after a press that travelled less than `SLOP` px changes nothing. The menu's buttons stay the
 * way to move a column from the keyboard. Nothing here runs per frame.
 */
export function installColumnDrag(shell: Shell): void {
  const { root } = shell;
  const head = optional<HTMLElement>(root, "table-head");
  const table = optional<HTMLElement>(root, "table");
  if (head === null || table === null) return;
  /** The head cell under the pointer, or null off the head. */
  const under = (e: PointerEvent): HTMLElement | null => (root.getRootNode() as Document | ShadowRoot).elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>(".cols > span") ?? null;
  const nameOf = (cell: Element | null): Movable | null => MOVABLE.find((name) => cell?.classList.contains(name)) ?? null;
  let drag: { name: Movable; x: number; marked: HTMLElement | null } | null = null;
  const mark = (cell: HTMLElement | null, side = ""): void => {
    if (drag?.marked && drag.marked !== cell) drag.marked.removeAttribute("data-drop");
    if (drag) drag.marked = cell;
    if (cell !== null) cell.dataset["drop"] = side;
  };
  const far = (e: PointerEvent): boolean => drag !== null && Math.abs(e.clientX - drag.x) >= SLOP;
  head.addEventListener("pointerdown", (e) => {
    const name = e.button === 0 ? nameOf((e.target as Element).closest(".cols > span")) : null;
    drag = name === null ? null : { name, x: e.clientX, marked: null };
    if (drag !== null) head.setPointerCapture(e.pointerId);
  });
  head.addEventListener("pointermove", (e) => {
    if (drag === null || !far(e)) return;
    const cell = under(e);
    const onto = nameOf(cell);
    const order = columnOrder(table);
    mark(onto === null || onto === drag.name ? null : cell, onto !== null && order.indexOf(onto) < order.indexOf(drag.name) ? "before" : "after");
  });
  head.addEventListener("pointerup", (e) => {
    const done = drag;
    const onto = done !== null && far(e) ? nameOf(under(e)) : null;
    mark(null);
    drag = null;
    if (done !== null && onto !== null && onto !== done.name) setColumnOrder(table, dropped(columnOrder(table), done.name, onto));
  });
  head.addEventListener("pointercancel", () => {
    mark(null);
    drag = null;
  });
}
