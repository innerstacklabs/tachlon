// The handlers form of `save` (decision 0022, rule 3.1): the queue hands a change set to `callSave`, and an object runs here, one handler per change, in order.
import type { ChangeSet, Handlers, Save, SaveContext, SaveResult } from "./gantt-types.js";

/** How long a handler runs before its `signal` aborts, in milliseconds; the chart goes on waiting for the handler to settle. */
const SIGNAL_AFTER = 10_000;

/** How many changes of a change set have had their handler resolve. It is kept with the set, so a later call with another object resumes at the change that failed. */
const resolved = new WeakMap<ChangeSet, number>();

/** The handlers that already warned that they resolved with an error object. */
const warned = new Set<string>();

type Name = Exclude<keyof Handlers, "classify">;
type Change = ChangeSet["changes"][number];

const nameOf = ({ record, op }: Change): Name => (record === "schedule" ? "updateSchedule" : `${op}${record === "task" ? "Task" : "Link"}`) as Name;

/** What a handler takes before its context: a create the row, an update the key and `set`, a delete the key, the schedule `set`. */
const argsOf = ({ record, op, key, set }: Change): unknown[] => (record === "schedule" || op === "create" ? [set] : op === "update" ? [key, set] : [key]);

/** Send a change set through `save`: a function is called with it, an object (the handlers form) runs its handlers, and stops before the next one once `stopped()` is true. */
export function callSave(save: Save | Handlers, changeSet: ChangeSet, stopped: () => boolean = () => false): Promise<SaveResult | void> {
  return typeof save === "function" ? save(changeSet) : runHandlers(save, changeSet, stopped);
}

async function runHandlers(handlers: Handlers, changeSet: ChangeSet, stopped: () => boolean): Promise<void> {
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
      result = await handler.apply(handlers, [...argsOf(change), context]);
    } finally {
      clearTimeout(timer);
    }
    if (result instanceof Response && !result.ok) throw result;
    warnIfRefused(name, result);
    resolved.set(changeSet, n + 1);
  }
}

/** A client that reports an error by resolving with it reads as saved, so the first time a handler does it the console says so. */
function warnIfRefused(name: Name, result: unknown): void {
  if (typeof result !== "object" || result === null || warned.has(name)) return;
  const member = ["error", "errors", "userErrors"].find((m) => m in result);
  if (member === undefined) return;
  warned.add(name);
  console.warn(`the ${name} handler resolved with an object that has "${member}"; the chart reads that as saved. Throw when the server refuses the change.`);
}
