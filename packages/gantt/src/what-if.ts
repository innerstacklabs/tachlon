// What moves if tasks slip a week (gantt#1250): `Commands.whatIf` asks `App.what_moves`, which solves a copy of the schedule, so the open
// schedule, its undo history and its events stay as they were. A bad ask is a result, never a throw.
import type { PublicApp } from "./api.js";

/** A task that would move: the engine's task number, its activity ID (`""` with none), its name, and its early start and early finish as `YYYY-MM-DD HH:MM`. */
export interface WhatIfMoved { task: number; id: string; name: string; startWas: string; startNow: string; finishWas: string; finishNow: string }

/** The project finish before and after, and the tasks that would move, in task order; or why nothing was asked. */
export type WhatIfResult =
  | { ok: true; finishWas: string; finishNow: string; moved: readonly WhatIfMoved[] }
  | { ok: false; reason: "busy" | "tooMany" | "unknownTask" | "summary" | "hasActuals" | "other"; message: string };

type Raw = { task: number; id: string; name: string; start_was: string; start_now: string; finish_was: string; finish_now: string };

/**
 * Ask `app` what moves if each of `tasks` (up to 10) slips a week.
 * @internal
 */
export function whatIf(app: Pick<PublicApp, "what_moves">, tasks: readonly number[]): WhatIfResult {
  // Index each position: `every` skips the holes of a sparse array, and `Uint32Array.from` would turn each into task 0.
  const named = Array.isArray(tasks) && Array.from({ length: tasks.length }, (_, i) => tasks[i]).every((n) => Number.isInteger(n) && n! >= 0 && n! <= 0xffff_ffff);
  if (!named) {
    return Object.freeze({ ok: false, reason: "unknownTask", message: "Name each task by its number." });
  }
  return answered(app.what_moves(Uint32Array.from(tasks)));
}

/**
 * Ask `app` what moves if `task` takes `days` working days longer; answered as `whatIf` answers a slip.
 * @internal
 */
export function whatIfLonger(app: Pick<PublicApp, "what_moves_longer">, task: number, days: number): WhatIfResult {
  const whole = (n: number): boolean => Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff;
  if (!whole(task) || !whole(days)) return Object.freeze({ ok: false, reason: "unknownTask", message: "Name the task and the days by whole numbers." });
  return answered(app.what_moves_longer(task, days));
}

/** @internal */
export function answered(json: string): WhatIfResult {
  const a = JSON.parse(json) as Extract<WhatIfResult, { ok: false }> | { ok: true; finish_was: string; finish_now: string; moved: Raw[] };
  if (!a.ok) return Object.freeze(a);
  const moved = a.moved.map((m) => Object.freeze({ task: m.task, id: m.id, name: m.name, startWas: m.start_was, startNow: m.start_now, finishWas: m.finish_was, finishNow: m.finish_now }));
  return Object.freeze({ ok: true, finishWas: a.finish_was, finishNow: a.finish_now, moved: Object.freeze(moved) });
}
