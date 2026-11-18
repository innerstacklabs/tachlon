/** How long a cell an `update` changed stays highlighted, in milliseconds. */
const CHANGED_MS = 1500;

export interface Changed {
  /** The next `before`/`after` pair compares the cells it draws. */
  arm: () => void;
  /** Call before the rows are replaced: keeps the texts shown, when armed. */
  before: () => void;
  /** Call after the rows are replaced: marks the cells whose text differs from the one kept (data-changed), and the ones marked before. */
  after: () => void;
  /** Another schedule: nothing is armed or marked any more. */
  drop: () => void;
}

/** Marks (data-changed) the cells of `rows` whose shown text an `update` changed; a timer clears them, and a redraw before then keeps them. */
export function trackChanged(rows: HTMLElement, onDispose: (undo: () => void) => void): Changed {
  let comparing = false;
  let kept: Map<string, string | null> | null = null;
  let changed: ReadonlySet<string> = new Set();
  let fade: ReturnType<typeof setTimeout> | undefined;
  onDispose(() => clearTimeout(fade));
  const cells = (): HTMLElement[] => [...rows.querySelectorAll<HTMLElement>(".row[data-task] > span")];
  const keyOf = (cell: HTMLElement): string => `${cell.parentElement?.dataset["task"]}/${cell.className.split(" ")[0]}`; // a task slot and a column
  return {
    arm: () => { comparing = true; },
    before: () => {
      kept = comparing ? new Map(cells().map((cell) => [keyOf(cell), cell.textContent])) : null;
      comparing = false;
    },
    after: () => {
      const shown = cells();
      const moved = kept === null ? [] : shown.filter((cell) => kept!.has(keyOf(cell)) && kept!.get(keyOf(cell)) !== cell.textContent);
      changed = new Set([...changed, ...moved.map(keyOf)]);
      for (const cell of shown) if (changed.has(keyOf(cell))) cell.dataset["changed"] = "true";
      if (moved.length === 0) return;
      clearTimeout(fade);
      fade = setTimeout(() => {
        changed = new Set();
        for (const cell of rows.querySelectorAll<HTMLElement>("[data-changed]")) delete cell.dataset["changed"];
      }, CHANGED_MS);
    },
    drop: () => {
      comparing = false;
      changed = new Set();
      clearTimeout(fade);
    },
  };
}
