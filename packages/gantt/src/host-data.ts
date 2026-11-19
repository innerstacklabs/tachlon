// `Gantt.setData` (decision 0022, rules 4.1 and 4.2): the host hands the chart its rows again, and the chart sets only the members that differ from the rows it was last given.
import type { PlainLink, PlainTask } from "./adapters/plain.js";
import { TYPES as LIMITS, spelledTwice } from "./constraint-form.ts";
import type { TaskUpdate, UpdateResult } from "./gantt-types.js";

/** The members of a task that `update` takes, the six limits among them (`update` answers `notUpdatable` for `mso` and `mfo`): the only ones `setData` compares. */
const MEMBERS = ["name", "start", "constraint", "duration", "progress", ...LIMITS] as const;
/** A member that differs from the one held; a `constraint` is an object, so it is compared by value. */
const differs = (given: unknown, held: unknown): boolean => given !== held && JSON.stringify(given) !== JSON.stringify(held);

/** The rows a host gives: either list may be left out. */
export interface HostRows { tasks?: PlainTask[]; links?: PlainLink[] }

/** `setData` of one start of the chart: `next` is the rows given, `epoch` the schedule's, and `apply` is the chart's `update`. */
export type SetData = (next: HostRows, epoch: number, apply: (records: TaskUpdate[]) => UpdateResult) => UpdateResult;

const rowsChanged = (message: string): UpdateResult => Object.freeze({ ok: false, reason: "rowsChanged", message: `Nothing changed: ${message}` });
const same = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((text, i) => text === b[i]);
const shapeOf = (tasks: readonly PlainTask[]): string[] => tasks.map((task) => JSON.stringify([task.key, task.id, task.parent]));
const linkTexts = (links: readonly PlainLink[]): string[] => links.map((link) => JSON.stringify(Object.entries(link).sort(([a], [b]) => (a < b ? -1 : 1))));

/** The compared members of each task by its `key`, or `undefined` when a task has no text `key` or two share one. */
function membersByKey(tasks: readonly PlainTask[]): Map<string, readonly unknown[]> | undefined {
  const byKey = new Map<string, readonly unknown[]>();
  for (const task of tasks) {
    if (typeof task.key !== "string" || byKey.has(task.key)) return undefined;
    byKey.set(task.key, MEMBERS.map((member) => task[member]));
  }
  return byKey;
}

const weak = (list: object | undefined): WeakRef<object> | undefined => list && new WeakRef(list);

/** Remember the rows a start of the chart was given, at schedule `epoch`; the function answers `setData` against what it last accepted. The page's own arrays are held weakly (the chart lets them go once it has started), only to skip the same array again. */
export function hostData(first: HostRows, epoch: number): SetData {
  let kept = { epoch, tasks: weak(first.tasks), byKey: first.tasks && membersByKey(first.tasks), shape: shapeOf(first.tasks ?? []), links: weak(first.links), linkTexts: linkTexts(first.links ?? []) };
  return (next, now, apply) => {
    const old = kept.byKey;
    if (old === undefined) return rowsChanged("the chart holds no rows of yours to compare with (every task needs its own text key).");
    if (kept.epoch !== now) return rowsChanged("a schedule was opened since the rows were given.");
    if (next.links !== undefined && next.links !== kept.links?.deref() &&!same(linkTexts(next.links), kept.linkTexts)) return rowsChanged("a link was added, removed or changed.");
    const byKey = new Map(old);
    const records: TaskUpdate[] = [];
    if (next.tasks !== undefined && next.tasks !== kept.tasks?.deref()) {
      const given = membersByKey(next.tasks);
      if (given === undefined) return rowsChanged("every task needs its own text key.");
      for (const task of next.tasks) {
        const twice = spelledTwice(task);
        if (twice !== undefined) return Object.freeze({ ok: false, reason: "badRecords", message: `Nothing changed: task ${task.key} gives both "constraint" and "${twice}", two spellings of one date limit; give one.` });
      }
      if (!same(shapeOf(next.tasks), kept.shape)) return rowsChanged("a task was added, removed or moved, or its id or parent changed.");
      for (const [key, values] of given) {
        const was = old.get(key)!;
        const set = MEMBERS.flatMap((member, i) => (values[i] !== undefined && differs(values[i], was[i]) ? [[member, values[i]] as const] : []));
        if (set.length > 0) records.push({ key, ...Object.fromEntries(set) });
        byKey.set(key, values.map((value, i) => (value === undefined ? was[i] : value)));
      }
    }
    const answer = records.length === 0 ? Object.freeze({ ok: true as const, updated: 0 }) : apply(records);
    if (answer.ok && next.tasks !== undefined) kept = { ...kept, tasks: weak(next.tasks), byKey };
    return answer;
  };
}
