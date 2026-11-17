// Why a task starts when it does (gantt#1250): `Commands.explain` asks `App.explain`, which reads the schedule's driving predecessors, so the open
// schedule, its undo history and its events stay as they were. A bad ask is a result, never a throw.
import type { PublicApp } from "./api.js";

/** A task in the chain: the engine's task number, its activity ID (`""` with none), its name, and its total float as the table writes it (`3d`). */
export interface ExplainLink { task: number; id: string; name: string; float: string }

/** The task's early start as `YYYY-MM-DD HH:MM` and the tasks that drive it, nearest first (`capped`: more exist than listed); or why nothing was asked. */
export type ExplainResult =
  | { ok: true; start: string; chain: readonly ExplainLink[]; capped: boolean }
  | { ok: false; reason: "busy" | "unknownTask"; message: string };

/**
 * Ask `app` why `task` starts when it does.
 * @internal
 */
export function explain(app: Pick<PublicApp, "explain">, task: number): ExplainResult {
  if (!Number.isInteger(task) || task < 0 || task > 0xffff_ffff) {
    return Object.freeze({ ok: false, reason: "unknownTask", message: "Name the task by its number." });
  }
  const a = JSON.parse(app.explain(task)) as ExplainResult;
  return Object.freeze(a.ok ? { ...a, chain: Object.freeze(a.chain.map((l) => Object.freeze(l))) } : a);
}
