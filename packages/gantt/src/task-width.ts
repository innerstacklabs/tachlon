/** Name indent per outline level, and the chevron slot every name keeps (a summary's button, a leaf's padding). Ledger: 14px each. */
export const INDENT_PX = 14;
export const CHEVRON_PX = 14;

/**
 * The width the Task column needs for the file's names: the widest of the longest name at each outline level, with the indent
 * and chevron slot of that level and the cell's own padding. `names[n]` is the longest name of level `n + 1` (`App.longest_name`).
 * Measured in the table's font by a hidden row, once per opened file, never per frame.
 */
export function measureTaskNeed(table: HTMLElement, names: string[]): number {
  const row = document.createElement("div");
  row.className = "row";
  row.style.cssText = "position:absolute;visibility:hidden;display:block;white-space:nowrap";
  const cell = document.createElement("span");
  cell.className = "name";
  cell.style.cssText = "position:static;display:block;width:max-content;overflow:visible";
  row.append(cell);
  table.append(row);
  let need = 0;
  names.forEach((name, level) => {
    if (name === "") return;
    cell.textContent = name;
    // The .name class has no inline-start padding of its own, so this replaces nothing (table.ts sets the same inline value on the rows).
    cell.style.paddingInlineStart = `${level * INDENT_PX + CHEVRON_PX}px`;
    need = Math.max(need, cell.getBoundingClientRect().width);
  });
  row.remove();
  return Math.ceil(need);
}

/**
 * The Task width to keep: what the names need, at least the token's minimum and at most `room`, the pane less everything
 * left of Task and the Dur, Start and Finish tracks with their gaps, so Start and Finish stay whole. The minimum wins over the cap.
 */
export const clampTaskNeed = (need: number, min: number, room: number): number => Math.max(min, Math.min(need, room));

/**
 * A pane edge through a column shows half a value (gs#851). Task is the one column that can give: widen it so the last column the pane
 * holds ends at the pane's right edge and the next starts past it. A column joins only while Task keeps `need`, the width the file's
 * longest name needs (measureTaskNeed), capped so Dur, Start and Finish stay whole (gs#959). When every column fits, or Task alone
 * does not, Task is left to its token, unless the names need more than it gives. Sets `--gantt-task-fit` on the root, or removes it.
 */
export function fitTaskWidth(root: HTMLElement, tableBody: HTMLElement, headCols: HTMLElement | null, need: number): void {
  const cells = [...(headCols?.children ?? [])] as HTMLElement[];
  const at = cells.findIndex((cell) => cell.classList.contains("name"));
  const width = tableBody.clientWidth;
  root.style.removeProperty("--gantt-task-fit");
  if (at < 0 || width <= 0 || !headCols) return;
  const min = parseFloat(getComputedStyle(root).getPropertyValue("--gantt-task-min"));
  const [head, cell] = [headCols.getBoundingClientRect(), cells[at]!.getBoundingClientRect()];
  // What lies before Task, from the inline start: the left edges left to right, the right edges right to left.
  const taskLeft = getComputedStyle(root).direction === "rtl" ? head.right - cell.right : cell.left - head.left;
  // Assumes Dur, Start and Finish follow .name in the head.
  const track = (k: number): number => 6 + (cells[at + k]?.getBoundingClientRect().width ?? 0);
  const keep = clampTaskNeed(need, min, width - taskLeft - track(1) - track(2) - track(3));
  let used = taskLeft; // the right edge of the last column that fits, less Task's own width
  let next = at + 1;
  for (; next < cells.length; next++) {
    const after = used + track(next - at);
    if (after + keep > width) break;
    used = after;
  }
  const fit = next < cells.length && used + min <= width ? Math.floor(width - used) : 0;
  const size = Math.max(fit, keep > min ? keep : 0);
  if (size > 0) root.style.setProperty("--gantt-task-fit", `${size}px`);
}
