// The tasks behind the schedule health counts (gantt#1250): `Commands.health` asks `App.health`, which lists, for each count of `checks()` that walks tasks,
// the first 50 tasks that count holds, so the open schedule, its undo history and its events stay as they were.
import type { PublicApp } from "./api.js";

/** A task a count holds: the engine's task number, its activity ID (`""` with none) and its name. */
export interface HealthTask { task: number; id: string; name: string }

/** One count: `count` is the true total (the number `checks()` reports) and `tasks` the first 50 of them in task order. */
export interface HealthCount { count: number; tasks: readonly HealthTask[] }

/** The counts that walk tasks, named as `checks()` names them; `invalidDates` and `missingResources` are `null` when that check did not run (no data date, no resource in the file). */
export interface HealthCounts {
  missingLogic: HealthCount; openStarts: HealthCount; openEnds: HealthCount; highFloat: HealthCount; highDuration: HealthCount;
  invalidDates: HealthCount | null; missingResources: HealthCount | null; missed: HealthCount; hardConstraints: HealthCount; negativeFloat: HealthCount;
}

/** The tasks behind each count; or `"busy"` while a bar or link is pressed or dragged. */
export type HealthResult = { ok: true; counts: Readonly<HealthCounts> } | { ok: false; reason: "busy"; message: string };

const count = (c: HealthCount): HealthCount => Object.freeze({ count: c.count, tasks: Object.freeze(c.tasks.map((t) => Object.freeze({ ...t }))) });
const maybe = (c: HealthCount | null): HealthCount | null => (c && count(c));

/**
 * Ask `app` which tasks sit behind each task-walking schedule health count.
 * @internal
 */
export function health(app: Pick<PublicApp, "health">): HealthResult {
  const a = JSON.parse(app.health()) as { ok: false; reason: "busy"; message: string } | { ok: true; counts: Record<string, HealthCount> };
  if (!a.ok) return Object.freeze(a);
  const c = a.counts;
  const counts: HealthCounts = {
    missingLogic: count(c.missing_logic), openStarts: count(c.open_starts), openEnds: count(c.open_ends), highFloat: count(c.high_float),
    highDuration: count(c.high_duration), invalidDates: maybe(c.invalid_dates), missingResources: maybe(c.missing_resources), missed: count(c.missed),
    hardConstraints: count(c.hard_constraints), negativeFloat: count(c.negative_float),
  };
  return Object.freeze({ ok: true, counts: Object.freeze(counts) });
}
