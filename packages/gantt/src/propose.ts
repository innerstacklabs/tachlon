// A proposal (gantt#1250): `Commands.propose` asks `App.propose_move`, `propose_duration`, `propose_progress`, `propose_link` or `propose_unlink`, which hold the change apart from the schedule while the chart
// draws a ghost where each task it moves would go. `acceptProposal` and `rejectProposal` end it, `acceptProposed` and `rejectProposed` settle one change of it. A bad ask is a result, never a throw.
import type { PublicApp } from "./api.js";
import { answered, type WhatIfResult } from "./what-if.ts"; // `.ts`, so the Node tier can load this file

/** One change to propose: a task starts no earlier than `start` (`YYYY-MM-DD` or `YYYY-MM-DD HH:MM`), or takes `days` working days of its calendar (0 makes a milestone), or reports one field of a task's progress (`actualStart` or `actualFinish` as `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`, a date alone being the first working minute of the day for a start and the end of its last for a finish, `remaining` in working days, `progress` as a percent from 0 to 100; give one), or the link from `pred` to `succ` is added or changed to `text` (`FS`, `SS+2d`), or removed. A second change to the same task, or the same pair, replaces the first. */
export type ProposedChange =
  | { kind: "move"; task: number; start: string }
  | { kind: "duration"; task: number; days: number }
  | { kind: "progress"; task: number; actualStart?: string; actualFinish?: string; remaining?: number; progress?: number }
  | { kind: "link"; pred: number; succ: number; text: string }
  | { kind: "unlink"; pred: number; succ: number };

/** Which held change `acceptProposed` and `rejectProposed` mean: the kind and the task, or the pair of tasks, of a `ProposedChange` (a held `"link"` or `"unlink"` of a pair is settled by either). */
export type ProposedKey = { kind: "move" | "duration" | "progress"; task: number } | { kind: "link" | "unlink"; pred: number; succ: number };

/** What `propose` answers: the project finish before and after and every task the held changes would move, as `WhatIfResult` says, or why the change was not held (`"badDate"`: the start is not a date; `"finishBeforeStart"`, `"actualAfterDataDate"`, `"duration"` and `"badPercent"`: a progress field the schedule refuses; `"badLinkText"`; `"loop"`; `"alreadyLinked"`: the live link is already that; `"noLink"`: there is none to remove). */
export type ProposeResult = WhatIfResult | { ok: false; reason: "badDate" | "finishBeforeStart" | "actualAfterDataDate" | "duration" | "badPercent" | "badLinkText" | "loop" | "alreadyLinked" | "noLink"; message: string };

const whole = (n: unknown): n is number => typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff;
const no = (reason: "unknownTask" | "badDate" | "badLinkText" | "other", message: string): ProposeResult => Object.freeze({ ok: false, reason, message });

type Loose = { kind?: unknown; task?: unknown; start?: unknown; days?: unknown; pred?: unknown; succ?: unknown; text?: unknown; actualStart?: unknown; actualFinish?: unknown; remaining?: unknown; progress?: unknown };
const FIELDS = { actualStart: "actual_start", actualFinish: "actual_finish", remaining: "remaining", progress: "percent" } as const;
const isLink = (k: Loose): boolean => k.kind === "link" || k.kind === "unlink";

/** The key as the app takes it; one that names nothing (not a kind, or not a task number) becomes a key no change holds. */
const taskKey = (k: Loose): [string, number] => ((k.kind === "move" || k.kind === "duration" || k.kind === "progress") && whole(k.task) ? [k.kind, k.task] : ["", 0]);

const pairKey = (k: Loose): [number, number] | undefined => (isLink(k) && whole(k.pred) && whole(k.succ) ? [k.pred, k.succ] : undefined);

/** Accept the held change `key` names, as the app answers it. @internal */
export const acceptKey = (app: PublicApp, key: ProposedKey): string => {
  const k = (key ?? {}) as Loose;
  const pair = pairKey(k);
  return pair ? app.accept_proposed_link(...pair) : app.accept_proposed(...taskKey(k));
};

/** Drop the held change `key` names; true when it was held. @internal */
export const rejectKey = (app: PublicApp, key: ProposedKey): boolean => {
  const k = (key ?? {}) as Loose;
  const pair = pairKey(k);
  return pair ? app.reject_proposed_link(...pair) : app.reject_proposed(...taskKey(k));
};

/**
 * Hold `change` with the changes already held (a second change to the same task, or the same pair of tasks, replaces the first) and ask `app` what they would move.
 * @internal
 */
export function propose(app: PublicApp, change: ProposedChange): ProposeResult {
  const c = (change ?? {}) as Loose;
  if (isLink(c)) {
    if (!whole(c.pred) || !whole(c.succ)) return no("unknownTask", "Name both tasks by their numbers.");
    if (c.kind === "unlink") return answered(app.propose_unlink(c.pred, c.succ)) as ProposeResult;
    return typeof c.text === "string" ? (answered(app.propose_link(c.pred, c.succ, c.text)) as ProposeResult) : no("badLinkText", "Write the link as FS, SS+2d or FF-1d.");
  }
  if (c.kind !== "move" && c.kind !== "duration" && c.kind !== "progress") return no("other", 'Propose a "move", a "duration", a "progress", a "link" or an "unlink".');
  if (!whole(c.task)) return no("unknownTask", "Name the task by its number.");
  if (c.kind === "progress") {
    const [f, ...more] = (Object.keys(FIELDS) as (keyof typeof FIELDS)[]).filter((name) => c[name] !== undefined);
    const value = f && !more.length ? c[f] : undefined;
    if (!f || (typeof value !== "string" && typeof value !== "number")) return no("other", "Give one of actualStart, actualFinish, remaining or progress.");
    return answered(app.propose_progress(c.task, FIELDS[f], f === "remaining" ? `${value}d` : String(value))) as ProposeResult;
  }
  if (c.kind === "move") {
    return typeof c.start === "string" ? (answered(app.propose_move(c.task, c.start)) as ProposeResult) : no("badDate", "Write the start as YYYY-MM-DD or YYYY-MM-DD HH:MM.");
  }
  return whole(c.days) ? (answered(app.propose_duration(c.task, c.days)) as ProposeResult) : no("other", "Give the days as a whole number.");
}
