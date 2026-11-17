/** The columns the Columns menu can move: the four after Task, in the order the table opens with. */
export const MOVABLE = ["dur", "start", "finish", "float"] as const;
export type Movable = (typeof MOVABLE)[number];

/** The table's flag for an order the menu has set: `data-column-order="start dur finish float"`. It is internal; the menu writes it, the cursor reads it. */
export const ORDER_FLAG = "data-column-order";

/** The grid track each column has. Start, Finish and Float are empty while the menu hides the column (columns.css). */
const TRACK: Record<Movable, string> = {
  dur: "60px",
  start: "var(--start-track, var(--gantt-date-col))",
  finish: "var(--finish-track, var(--gantt-date-col))",
  float: "var(--float-track, 46px)",
};

/** The table's order of the four columns; the default when the table has no flag or the flag does not name each once. */
export function columnOrder(table: Element): readonly Movable[] {
  const named = (table.getAttribute(ORDER_FLAG) ?? "").split(" ");
  return named.length === MOVABLE.length && MOVABLE.every((column) => named.includes(column)) ? (named as Movable[]) : MOVABLE;
}

/**
 * Put the four columns in `order`, for the rest of the visit: the flag for the cursor, then a position per column (`--order-<column>`, which
 * column-order.css turns into `order`) and the tracks in that order (`--track-1` to `--track-4`), so the head and every row follow.
 */
export function setColumnOrder(table: HTMLElement, order: readonly Movable[]): void {
  table.setAttribute(ORDER_FLAG, order.join(" "));
  order.forEach((column, at) => {
    table.style.setProperty(`--order-${column}`, String(at + 1));
    table.style.setProperty(`--track-${at + 1}`, TRACK[column]);
  });
}

/** `order` with `column` one place earlier (-1) or later (1); the same order at either end. */
export function moved(order: readonly Movable[], column: Movable, by: -1 | 1): Movable[] {
  const next = [...order];
  const at = next.indexOf(column);
  const to = at + by;
  if (at >= 0 && to >= 0 && to < next.length) [next[at], next[to]] = [next[to]!, next[at]!];
  return next;
}

/** `order` with `column` put where `onto` stands: it takes `onto`'s place and the columns between shift one place the other way; unchanged if either is not in `order`. */
export function dropped(order: readonly Movable[], column: Movable, onto: Movable): Movable[] {
  const to = order.indexOf(onto);
  if (to < 0 || !order.includes(column)) return [...order];
  const next = order.filter((name) => name !== column);
  next.splice(to, 0, column);
  return next;
}

/** `names` with the four movable columns, wherever they stand in it, taking the table's order in their places. */
export function arranged<T extends string>(names: readonly T[], table: Element | null): T[] {
  const order = table === null ? MOVABLE : columnOrder(table);
  let at = 0;
  return names.map((name) => ((MOVABLE as readonly string[]).includes(name) ? (order[at++] as string as T) : name));
}
