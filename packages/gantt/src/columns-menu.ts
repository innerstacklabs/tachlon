import { columnOrder, moved, setColumnOrder, type Movable } from "./column-order.js";
import { optional } from "./dom.js";
import { listening, type Shell } from "./shell.js";

/** The columns the menu can hide, by the class of their cells. Task and Dur stay: Task names the row, and Dur is the cell an edit walks to. */
export const HIDEABLE = ["start", "finish", "float"] as const;
/** The table's flag for a hidden column: `data-hide-start`, `data-hide-finish` or `data-hide-float` (table.css leaves its track, head and cells out). */
export const HIDE_FLAGS: readonly string[] = HIDEABLE.map((column) => `data-hide-${column}`);

/** Whether the menu has hidden the column whose cells carry the class `column`. */
export const isHidden = (table: Element, column: string): boolean => table.hasAttribute(`data-hide-${column}`);

/**
 * The panel's move buttons: a press moves the column of its row one place up or down the list and sets the table's order to match
 * (column-order.ts). A button at the end of the list does nothing (aria-disabled, not disabled, so focus stays where it is). Returns the
 * function that puts the rows in the table's order and marks those ends; a page can set the order itself, so the panel runs it at each open.
 */
function installMoves(panel: HTMLElement, table: HTMLElement): () => void {
  const buttons = [...panel.querySelectorAll<HTMLButtonElement>("button[data-move]")];
  const rowOf = (item: HTMLElement): Movable => item.closest<HTMLElement>("[data-row]")?.dataset.row as Movable;
  const arrange = (): void => {
    const order = columnOrder(table);
    panel.append(...order.map((column) => panel.querySelector(`[data-row=${column}]`)).filter((row) => row !== null));
    for (const item of buttons) item.setAttribute("aria-disabled", String(order.indexOf(rowOf(item)) === (item.dataset.move === "-1" ? 0 : order.length - 1)));
  };
  for (const item of buttons) {
    item.addEventListener("click", () => {
      if (item.getAttribute("aria-disabled") === "true") return;
      setColumnOrder(table, moved(columnOrder(table), rowOf(item), item.dataset.move === "-1" ? -1 : 1));
      arrange();
      item.focus({ preventScroll: true });
    });
  }
  return arrange;
}

/**
 * The Columns menu: a button in the toolbar that opens a panel with one checkbox row for each of Start, Finish and Float, checked while the
 * column shows. A press sets or removes `data-hide-<column>` on the table, as the Variance button sets `data-variance-hidden`;
 * the stylesheet does the rest. Each of Dur, Start, Finish and Float also has two buttons that move it up or down the list, and the table
 * puts the column left or right in step (column-order.ts), for the rest of the visit; nothing is stored. Nothing here runs per frame.
 * Enter or Space on the button opens the panel and puts focus on its first item; the arrows walk the items, Escape closes it and puts
 * focus back on the button, and a press outside it, or focus leaving it, closes it. The chart works with the toolbar removed:
 * without the button there is nothing to press.
 */
export function installColumnsMenu(shell: Shell): void {
  const { root } = shell;
  const button = optional<HTMLButtonElement>(root, "columns-button");
  const panel = optional<HTMLElement>(root, "columns-panel");
  const table = optional<HTMLElement>(root, "table");
  if (button === null || panel === null || table === null) return;
  const checks = [...panel.querySelectorAll<HTMLButtonElement>("button[data-column]")];
  // The rows follow the list's order, so the items are read in the order they stand in now.
  const items = (): HTMLButtonElement[] => [...panel.querySelectorAll<HTMLButtonElement>("button")];
  const arrange = installMoves(panel, table);
  const close = (refocus: boolean): void => {
    if (panel.hidden) return;
    panel.hidden = true;
    button.setAttribute("aria-expanded", "false");
    if (refocus) button.focus({ preventScroll: true });
  };
  button.addEventListener("click", () => {
    if (!panel.hidden) {
      close(true);
      return;
    }
    panel.hidden = false;
    // A page can set the flags itself (the hosted page restores the last visit's), so the rows read them when the panel opens.
    for (const item of checks) item.setAttribute("aria-checked", String(!isHidden(table, item.dataset.column ?? "")));
    arrange();
    button.setAttribute("aria-expanded", "true");
    items()[0]?.focus({ preventScroll: true });
  });
  for (const item of checks) {
    item.addEventListener("click", () => item.setAttribute("aria-checked", String(!table.toggleAttribute(`data-hide-${item.dataset.column}`))));
  }
  const within = (node: Node): boolean => button.contains(node) || panel.contains(node);
  // The keys are the menu's, not the chart's: Escape here must not also clear the chart's selection, nor Enter open the selected cell's editor.
  const keys = (e: KeyboardEvent): void => {
    if (e.target === button && (e.key === "Enter" || e.key === " ")) e.stopPropagation();
    if (e.key === "Escape" && !panel.hidden) {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    const all = items();
    const at = all.indexOf(e.target as HTMLButtonElement);
    const next = { ArrowDown: at + 1, ArrowRight: at + 1, ArrowUp: at - 1, ArrowLeft: at - 1, Home: 0, End: all.length - 1 }[e.key];
    if (at < 0 || next === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    all[(next + all.length) % all.length]?.focus({ preventScroll: true });
  };
  for (const el of [button, panel]) {
    el.addEventListener("keydown", keys);
    el.addEventListener("focusout", (e) => {
      if (e.relatedTarget instanceof Node && !within(e.relatedTarget)) close(false);
    });
  }
  listening(shell, root, "pointerdown", (e: PointerEvent) => {
    if (!(e.target instanceof Node) || !within(e.target)) close(false);
  });
}
