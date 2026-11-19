// The handlers form of `save` (decision 0022, rule 3.1): the queue hands a change set to `callSave`, and an object runs here, one handler per change, in order.
import type { ChangeSet, Handlers, Save, SaveContext, SaveResult } from "./gantt-types.js";

/** How long a handler runs before its `signal` aborts, in milliseconds; the chart goes on waiting for the handler to settle. */
const SIGNAL_AFTER = 10_000;

/** How many changes of a change set have had their handler resolve. It is kept with the set, so a later call with another object resumes at the change that failed. */
const resolved = new WeakMap<ChangeSet, number>();

/** The key each create of a change set that carries a `ref` has been answered with, kept with the set for the same reason. */
const answered = new WeakMap<ChangeSet, Map<string, string>>();

/** A handler resolved with something the chart cannot read a key from (`keys: "server"`): the chart puts the edit back with the reason `"result"`. */
export class BadResult extends Error {}

/** The host's name for the key of a task and of a link, given when the server makes the keys. */
export interface KeyNames { task: string; link: string }

/** The handlers that already warned that they resolved with an error object. */
const warned = new Set<string>();

type Name = Exclude<keyof Handlers, "classify">;
type Change = ChangeSet["changes"][number];

const nameOf = ({ record, op }: Change): Name => (record === "schedule" ? "updateSchedule" : `${op}${record === "task" ? "Task" : "Link"}`) as Name;

/** What a handler takes before its context: a create the row, an update the key and `set`, a delete the key, the schedule `set`. Each `{ ref }` in a row is the key its create answered. */
const argsOf = ({ record, op, key, set }: Change, keys: Map<string, string>): unknown[] => {
  const row = set && Object.fromEntries(Object.entries(set).map(([m, v]) => [m, keys.get((v as { ref?: string } | null)?.ref ?? "") ?? v]));
  return record === "schedule" || op === "create" ? [row] : op === "update" ? [key, row] : [key];
};

/** Send a change set through `save`: a function is called with it, an object (the handlers form) runs its handlers, and stops before the next one once `stopped()` is true. */
export function callSave(save: Save | Handlers, changeSet: ChangeSet, stopped: () => boolean = () => false, names?: KeyNames): Promise<SaveResult | void> {
  return typeof save === "function" ? save(changeSet) : runHandlers(save, changeSet, stopped, names);
}

/** The key a create's handler answered: the key itself, an object that holds it under the host's name for `key`, or a `Response` whose JSON is that row. */
async function keyOf(result: unknown, field: string, name: Name): Promise<string> {
  const row = result instanceof Response ? (await result.json().catch(() => undefined)) as unknown : result;
  const key = typeof row === "object" && row !== null ? (row as Record<string, unknown>)[field] : row;
  if (typeof key === "string" && key !== "") return key;
  throw new BadResult(`the ${name} handler resolved with no key for the new row: with keys: "server" resolve with the key, an object that has it as "${field}", or a Response whose JSON body is the row`);
}

async function runHandlers(handlers: Handlers, changeSet: ChangeSet, stopped: () => boolean, names?: KeyNames): Promise<SaveResult | void> {
  const keys = answered.get(changeSet) ?? new Map<string, string>();
  answered.set(changeSet, keys);
  for (let n = resolved.get(changeSet) ?? 0; n < changeSet.changes.length; n++) {
    if (stopped()) return;
    const change = changeSet.changes[n]!;
    const name = nameOf(change);
    const handler = handlers[name] as ((...args: unknown[]) => Promise<unknown>) | undefined;
    if (handler === undefined) throw new Error(`the save handlers have no ${name}`);
    const abort = new AbortController();
    const context: SaveContext = { once: change.once, signal: abort.signal, changeSet, ...(change.was === undefined ? {} : { was: change.was }) };
    const timer = setTimeout(() => abort.abort(), SIGNAL_AFTER);
    let result: unknown;
    try {
      result = await handler.apply(handlers, [...argsOf(change, keys), context]);
    } finally {
      clearTimeout(timer);
    }
    if (result instanceof Response && !result.ok) throw result;
    warnIfRefused(name, result);
    if (names !== undefined && change.ref !== undefined) keys.set(change.ref, await keyOf(result, names[change.record as "task" | "link"], name));
    resolved.set(changeSet, n + 1);
  }
  if (keys.size > 0) return { keys: [...keys].map(([ref, key]) => ({ ref, key })) };
}

/** A client that reports an error by resolving with it reads as saved, so the first time a handler does it the console says so. */
function warnIfRefused(name: Name, result: unknown): void {
  if (typeof result !== "object" || result === null || warned.has(name)) return;
  const member = ["error", "errors", "userErrors"].find((m) => m in result);
  if (member === undefined) return;
  warned.add(name);
  console.warn(`the ${name} handler resolved with an object that has "${member}"; the chart reads that as saved. Throw when the server refuses the change.`);
}
