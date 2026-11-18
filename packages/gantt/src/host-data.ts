// `Gantt.setData` (decision 0022, rules 4.1 and 4.2): the host hands the chart its rows again, and the chart sets only the members that differ from the rows it was last given.
import type { PlainLink, PlainTask } from "./adapters/plain.js";
import type { TaskUpdate, UpdateResult } from "./gantt-types.js";

/** The members of a task that `update` takes: the only ones `setData` compares. */
const MEMBERS = ["name", "start", "snet", "duration", "progress"] as const;

/** The rows a host gives: either list may be left out. */
export interface HostRows { tasks?: PlainTask[]; links?: PlainLink[] }

/** `setData` of one start of the chart: `next` is the rows given, `epoch` the schedule's, and `apply` is the chart's `update`. */
export type SetData = (next: HostRows, epoch: number, apply: (records: TaskUpdate[]) => UpdateResult) => UpdateResult;

const rowsChanged = (message: string): UpdateResult => Object.freeze({ ok: false, reason: "rowsChanged", message: `Nothing changed: ${message}` });
const same = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((text, i) => text === b[i]);
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

/** Remember the rows a start of the chart was given, at schedule `epoch`; the function answers `setData` against what it last accepted. */
export function hostData(first: HostRows, epoch: number): SetData {
  let kept = { epoch, tasks: first.tasks, byKey: first.tasks && membersByKey(first.tasks), links: first.links ?? [], linkTexts: linkTexts(first.links ?? []) };
  return (next, now, apply) => {
    const old = kept.byKey;
    if (old === undefined) return rowsChanged("the chart holds no rows of yours to compare with (every task needs its own text key).");
    if (kept.epoch !== now) return rowsChanged("a schedule was opened since the rows were given.");
    if (next.links !== undefined && next.links !== kept.links && !same(linkTexts(next.links), kept.linkTexts)) return rowsChanged("a link was added, removed or changed.");
    const byKey = new Map(old);
    const records: TaskUpdate[] = [];
    if (next.tasks !== undefined && next.tasks !== kept.tasks) {
      const given = membersByKey(next.tasks);
      if (given === undefined) return rowsChanged("every task needs its own text key.");
      if (given.size !== old.size || [...given.keys()].some((key) => !old.has(key))) return rowsChanged("a task was added or removed.");
      for (const [key, values] of given) {
        const was = old.get(key)!;
        const set = MEMBERS.flatMap((member, i) => (values[i] !== undefined && values[i] !== was[i] ? [[member, values[i]] as const] : []));
        if (set.length > 0) records.push({ key, ...Object.fromEntries(set) });
        byKey.set(key, values.map((value, i) => (value === undefined ? was[i] : value)));
      }
    }
    const answer = records.length === 0 ? Object.freeze({ ok: true as const, updated: 0 }) : apply(records);
    if (answer.ok && next.tasks !== undefined) kept = { ...kept, tasks: next.tasks, byKey };
    return answer;
  };
}
