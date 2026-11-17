// The critical tasks and the longest path (gantt#1250): `Commands.criticalPath` asks `App.critical_path`, which lists both, so the open schedule, its undo history and its events stay as they were.
import type { PublicApp } from "./api.js";
import type { HealthTask } from "./health.js";

/** One list: `count` is the true total and `tasks` the first 50 of them in task order. */
export interface CriticalList { count: number; tasks: readonly HealthTask[] }

/** The tasks with zero total float that are not finished (`critical`) and the tasks of the longest path (`longest`, which can be fewer); neither holds a summary. Or `"busy"` while a bar or link is pressed or dragged. */
export type CriticalPathResult = { ok: true; critical: CriticalList; longest: CriticalList } | { ok: false; reason: "busy"; message: string };

const list = (l: CriticalList): CriticalList => Object.freeze({ count: l.count, tasks: Object.freeze(l.tasks.map((t) => Object.freeze({ ...t }))) });

/**
 * Ask `app` which tasks are critical and which make up the longest path.
 * @internal
 */
export function criticalPath(app: Pick<PublicApp, "critical_path">): CriticalPathResult {
  const a = JSON.parse(app.critical_path()) as { ok: false; reason: "busy"; message: string } | { ok: true; critical: CriticalList; longest: CriticalList };
  if (!a.ok) return Object.freeze(a);
  return Object.freeze({ ok: true, critical: list(a.critical), longest: list(a.longest) });
}
