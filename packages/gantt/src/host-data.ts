// `Gantt.setData` (decision 0022, rules 4.1 and 4.2): the host hands the chart its rows again, and the chart sets only the members that differ from the rows it was last given and adds the tasks it lacks.
import type { PlainLink, PlainTask } from "./adapters/plain.js";
import { MEMBERS as LIMITS, spelledTwice } from "./constraint-form.ts";
import type { TaskUpdate, UpdateResult } from "./gantt-types.js";

/** The members of a task that `update` takes, the six limits among them (`update` answers `notUpdatable` for an `mso` or `mfo` that is not the pin the chart holds): the only ones `setData` compares. */
const MEMBERS = ["name", "start", "constraint", "duration", "progress", "actualStart", "actualFinish", "remaining", ...LIMITS] as const;
/** A member that differs from the one held; a `constraint` is an object, so it is compared by value. */
const differs = (given: unknown, held: unknown): boolean => given !== held && JSON.stringify(given) !== JSON.stringify(held);

/** The rows a host gives and the day they are as of: any of the three may be left out. */
export interface HostRows { tasks?: PlainTask[]; links?: PlainLink[]; dataDate?: string }

/** A task the data holds and the chart does not: its `record` (the key and the members `update` takes), the key of the row it goes under (none: the top level) and of the one it follows (none: it goes first). */
export interface Add { record: TaskUpdate; parent?: string; after?: string }
/** `setData` of one start of the chart: `next` is the rows given, `epoch` the schedule's, `apply` moves the data date to `day` (when there is one) with the chart's `setDataDate`, sets the records with its `update`, then adds the tasks it lacks, in the order given. A refusal it answers has `added`, how many adds the chart took before it, once it took the records (none when it refused them); `setData` does not pass `added` on. */
export type SetData = (next: HostRows, epoch: number, apply: (records: TaskUpdate[], day: string | undefined, adds: Add[]) => UpdateResult & { added?: number }) => UpdateResult;

export const rowsChanged = (message: string): UpdateResult => Object.freeze({ ok: false, reason: "rowsChanged", message: `Nothing changed: ${message}` });
const same = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((text, i) => text === b[i]);

/** Where the tasks of a list sit: `at` is each one's `id` and its parent's key by its own key, `kids` the keys under each parent's key (`null`: the top level) in the order given. */
interface Layout { at: Map<string, string>; kids: Map<string | null, string[]> }
type Place = Omit<Add, "record"> & { key: string };

/** The layout of tasks that each have a text key; undefined when a `parent` names no task. A `parent` is read as `fromTasks` reads it: none, `null`, `""` and `0` are the top level. */
function layoutOf(tasks: readonly PlainTask[]): Layout | undefined {
  const keyOf = new Map(tasks.map((task) => [String(task.id), task.key!]));
  const layout: Layout = { at: new Map(), kids: new Map() };
  for (const { key, id, parent } of tasks) {
    const under = parent === undefined || parent === null || parent === "" || String(parent) === "0" ? null : keyOf.get(String(parent));
    if (under === undefined) return undefined;
    layout.at.set(key!, JSON.stringify([id, under]));
    const list = layout.kids.get(under);
    if (list === undefined) layout.kids.set(under, [key!]);
    else list.push(key!);
  }
  return layout;
}

/** The tasks `next` has and `held` lacks, in outline order (a parent before its children), each with the key of the row it goes under and of the one it follows; or why `next` is more than adds: a task removed or moved, another `id` or `parent`, or parents that form a loop. */
function placesOf(held: Layout, next: Layout): string | Place[] {
  if ([...held.at.keys()].some((key) => !next.at.has(key))) return "a task was removed.";
  if ([...held.at].some(([key, at]) => next.at.get(key) !== at)) return "a task has another id or parent.";
  if ([...held.kids].some(([parent, keys]) => !same(next.kids.get(parent)?.filter((key) => held.at.has(key)) ?? [], keys))) return "a task was moved in the list.";
  const places: Place[] = [];
  const walk = (parent: string | null): number => (next.kids.get(parent) ?? []).reduce((count, key, i, list) => {
    if (!held.at.has(key)) places.push({ key, parent: parent ?? undefined, after: list[i - 1] });
    return count + 1 + walk(key);
  }, 0);
  return walk(null) === next.at.size ? places : "the parents of the tasks form a loop.";
}
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

/** True when a task of `next` has no text `key`, or differs from the task of `held` with its key in a member `setData` ignores (`color`, `end` and the rest, the host's own included), or is a task `held` lacks that gives one: the rows must start the chart again, or the change is dropped. A changed `id` or `parent` is `setData`'s to refuse. */
export function unsetChange(held: readonly PlainTask[], next: readonly PlainTask[]): boolean {
  const before = new Map(held.map((task) => [task.key, task]));
  const taken = new Set<string>([...MEMBERS, "key", "id", "parent"]);
  return next.some((task) => {
    const was = typeof task.key === "string" ? before.get(task.key) : undefined;
    return typeof task.key !== "string" || (was === undefined ? Object.keys(task).some((member) => !taken.has(member) && task[member] !== undefined) : MEMBERS.some((member) => was[member] !== undefined && task[member] === undefined) || [...new Set([...Object.keys(was), ...Object.keys(task)])].some((member) => !taken.has(member) && differs(task[member], was[member])));
  });
}

/** `text` as the day `setDataDate` reads (`YYYY-MM-DD`, trimmed, a day the calendar has), or undefined. */
const dayOf = (text: unknown): string | undefined => {
  const day = typeof text === "string" ? text.trim() : "";
  return /^\d{4}-\d{2}-\d{2}$/.test(day) && new Date(`${day}T00:00:00Z`).toJSON()?.slice(0, 10) === day ? day : undefined;
};

const weak = (list: object | undefined): WeakRef<object> | undefined => list && new WeakRef(list);

/** Remember the rows a start of the chart was given, at schedule `epoch`; the function answers `setData` against what it last accepted. The page's own arrays are held weakly (the chart lets them go once it has started), only to skip the same array again. Rows without a key for every task can only be refused, so their layout and links are not read. */
export function hostData(first: HostRows, epoch: number): SetData {
  const keyed = first.tasks && membersByKey(first.tasks);
  const laid = keyed ? layoutOf(first.tasks ?? []) : undefined;
  let kept = { epoch, tasks: weak(first.tasks), byKey: laid && keyed, layout: laid, links: weak(first.links), linkTexts: keyed ? linkTexts(first.links ?? []) : [], day: first.dataDate?.slice(0, 10) };
  return (next, now, apply) => {
    const old = kept.byKey;
    const held = kept.layout;
    if (old === undefined || held === undefined) return rowsChanged("the chart holds no rows of yours to compare with (every task needs its own text key).");
    if (kept.epoch !== now) return rowsChanged("a schedule was opened since the rows were given.");
    if (next.links !== undefined && next.links !== kept.links?.deref() &&!same(linkTexts(next.links), kept.linkTexts)) return rowsChanged("a link was added, removed or changed.");
    const byKey = new Map(old);
    const records: TaskUpdate[] = [];
    let adds: Add[] = [];
    let layout = held;
    if (next.tasks !== undefined && next.tasks !== kept.tasks?.deref()) {
      const given = membersByKey(next.tasks);
      if (given === undefined) return rowsChanged("every task needs its own text key.");
      for (const task of next.tasks) {
        const twice = spelledTwice(task);
        if (twice !== undefined) return Object.freeze({ ok: false, reason: "badRecords", message: `Nothing changed: task ${task.key} gives both "constraint" and "${twice}", two spellings of one date limit; give one.` });
      }
      const to = layoutOf(next.tasks);
      if (to === undefined) return rowsChanged("a task's parent is no task of the rows.");
      const places = placesOf(held, to);
      if (typeof places === "string") return rowsChanged(places);
      const setOf = (values: readonly unknown[], was?: readonly unknown[]) => MEMBERS.flatMap((member, i) => (values[i] !== undefined && (was === undefined || differs(values[i], was[i])) ? [[member, values[i]] as const] : []));
      layout = to;
      adds = places.map(({ key, ...place }) => ({ ...place, record: { key, ...Object.fromEntries(setOf(given.get(key)!)) } }));
      for (const [key, values] of given) {
        const was = old.get(key);
        const set = setOf(values, was);
        if (was !== undefined && set.length > 0) records.push({ key, ...Object.fromEntries(set) });
        byKey.set(key, values.map((value, i) => (value === undefined ? was?.[i] : value)));
      }
    }
    const day = next.dataDate === undefined ? undefined : dayOf(next.dataDate);
    if (next.dataDate !== undefined && day === undefined) return Object.freeze({ ok: false, reason: "badDate", message: "Nothing changed: write the data date as YYYY-MM-DD." });
    const move = day !== kept.day ? day : undefined;
    const { added, ...answer } = records.length === 0 && adds.length === 0 && move === undefined ? { ok: true as const, updated: 0 } : apply(records, move, adds);
    const took = answer.ok ? adds.length : added;
    if (took !== undefined) { // the chart took the records: the rows it was given and the first `took` adds are held, and what it refused is offered again
      adds.slice(took).forEach(({ record }) => byKey.delete(record.key));
      kept = { ...kept, byKey, layout: took === adds.length ? layout : layoutOf((next.tasks ?? []).filter(({ key }) => byKey.has(key!))), ...(answer.ok && next.tasks !== undefined && { tasks: weak(next.tasks) }), ...(move !== undefined && { day: move }) };
    }
    return Object.freeze(answer) as UpdateResult;
  };
}
