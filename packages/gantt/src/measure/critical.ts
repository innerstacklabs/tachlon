import type { TableRow } from "../api.js";
import type { MeasureConfig } from "./config.js";

/**
 * The leaf rows a file's drag may use. With `dragTarget` `"critical"` it is the one earliest-starting critical leaf
 * (`start(row)`, ties by row), chosen by the rule the engine's `drag_finish` benches use (the first critical task by
 * start) but on the loaded file, not the benches' generated schedule: delaying it moves the project finish.
 */
export function dragCandidates(rows: readonly TableRow[], target: MeasureConfig["dragTarget"], start: (row: number) => number): number[] {
  const leaves = rows.flatMap((t, row) => (t[5] === 0 && (target === "head" || t[4]) ? [row] : []));
  return target === "critical" ? leaves.sort((a, b) => start(a) - start(b) || a - b).slice(0, 1) : leaves;
}
