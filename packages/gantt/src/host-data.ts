// `Gantt.setData` (decision 0022, rules 4.1 and 4.2): the host hands the chart its rows again, and the chart sets only the members that differ from the rows it was last given and adds the tasks it lacks.
import { barLook } from "./adapters/dhtmlx.ts";
import type { PlainLink, PlainTask } from "./adapters/plain.js";
import { MEMBERS as LIMITS, spelledTwice } from "./constraint-form.ts";
import type { TaskUpdate, UpdateResult } from "./gantt-types.js";

/** The members of a task that `update` takes, the six limits among them (`update` answers `notUpdatable` for an `mso` or `mfo` that is not the pin the chart holds): the only ones `setData` compares. */
const MEMBERS = ["name", "start", "constraint", "duration", "progress", "actualStart", "actualFinish", "remaining", ...LIMITS] as const;
/** The members of an added task that its record carries beside the ones `update` takes: the bar's `color` and `label`. A kept task's are compared by no one: `unsetChange` has the components start the chart again for a change. */
const LOOK = ["color", "label"] as const;
/** The look of an added task as `mount` reads it: a `color` that is not `#rrggbb` or a `label` that is not text is left out. */
const lookOf = (task: PlainTask): Pick<TaskUpdate, (typeof LOOK)[number]> => barLook(task, new Map(), true);
/** A member that differs from the one held; a `constraint` is an object, so it is compared by value. */
const differs = (given: unknown, held: unknown): boolean => given !== held && JSON.stringify(given) !== JSON.stringify(held);

/** The rows a host gives and the day they are as of: any of the three may be left out. */
export interface HostRows { tasks?: PlainTask[]; links?: PlainLink[]; dataDate?: string }

/** A task the data holds and the chart does not: its `record` (the key and the members `update` takes), the key of the row it goes under (none: the top level) and of the one it follows (none: it goes first). */
export interface Add { record: TaskUpdate; parent?: string; after?: string }
/** A link the data holds and the chart lacks (to add), or the reverse (to drop): its ends as the keys of tasks, with the `key`, `type` and `lag` (in the data's `durationUnit`) the data gives it. */
export interface LinkChange { pred: string; succ: string; key?: string; type?: PlainLink["type"]; lag?: number }
/** The links `setData` adds, after the tasks it adds, and drops, before the tasks it drops (a link of a dropped task goes with the task and is not listed). */
export interface LinkChanges { adds: LinkChange[]; drops: LinkChange[] }
/** `setData` of one start of the chart: `next` is the rows given, `epoch` the schedule's, `apply` moves the data date to `day` (when there is one) with the chart's `setDataDate`, sets the records with its `update`, adds the tasks it lacks, in the order given, then drops the links in `links.drops`, adds those in `links.adds`, and drops the keys in `drops` (the rows the data no longer holds, the deepest first). A refusal it answers has `added`, how many adds the chart took before it, `unlinked` and `linked`, how many link drops and adds (none: 0), and `dropped`, how many drops (none: 0), once it took the records (none when it refused them); `setData` does not pass them on. An answer that went as far as the drops also has `skipped`, the keys among the drops it took that the chart held no live task of (a row a person deleted): `setData` holds those rows until the chart holds them again. `live` (all live when left out) says for each key whether the chart holds a live task of it. */
export type SetData = (next: HostRows, epoch: number, apply: (records: TaskUpdate[], day: string | undefined, adds: Add[], drops: string[], links: LinkChanges) => UpdateResult & { added?: number; dropped?: number; skipped?: string[]; linked?: number; unlinked?: number }, live?: (keys: string[]) => boolean[]) => UpdateResult;

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

/** The tasks `next` has and `held` lacks, in outline order (a parent before its children), each with the key of the row it goes under and of the one it follows; or why `next` is more than adds and drops: a task moved, another `id` or `parent` (a kept child of a dropped row has one), or parents that form a loop. */
function placesOf(held: Layout, next: Layout): string | Place[] {
  if ([...held.at].some(([key, at]) => next.at.has(key) && next.at.get(key) !== at)) return "a task has another id or parent.";
  if ([...held.kids].some(([parent, keys]) => !same(next.kids.get(parent)?.filter((key) => held.at.has(key)) ?? [], keys.filter((key) => next.at.has(key))))) return "a task was moved in the list.";
  const places: Place[] = [];
  const walk = (parent: string | null): number => (next.kids.get(parent) ?? []).reduce((count, key, i, list) => {
    if (!held.at.has(key)) places.push({ key, parent: parent ?? undefined, after: list[i - 1] });
    return count + 1 + walk(key);
  }, 0);
  return walk(null) === next.at.size ? places : "the parents of the tasks form a loop.";
}
/** The keys `held` has and `next` lacks, the deepest first (rows of one depth in the order they are held). */
function dropsOf(held: Layout, next: Layout): string[] {
  const depth = (key: string): number => { const up = (JSON.parse(held.at.get(key)!) as [unknown, string | null])[1]; return up === null ? 0 : 1 + depth(up); };
  return [...held.at.keys()].filter((key) => !next.at.has(key)).map((key) => [key, depth(key)] as const).sort((a, b) => b[1] - a[1]).map(([key]) => key);
}
/** `layout` with the rows of `rest` put back where `held` has them: in their parents' lists before the next row of the list that `layout` holds. */
function withRest(layout: Layout | undefined, held: Layout, rest: readonly string[]): Layout | undefined {
  for (const key of rest) {
    const at = held.at.get(key)!;
    const parent = (JSON.parse(at) as [unknown, string | null])[1];
    const list = layout?.kids.get(parent) ?? [];
    const was = held.kids.get(parent)!;
    const later = was.slice(was.indexOf(key) + 1).find((other) => list.includes(other));
    list.splice(later === undefined ? list.length : list.indexOf(later), 0, key);
    layout?.at.set(key, at);
    layout?.kids.set(parent, list);
  }
  return layout;
}
/** `layout` without the rows of `gone` (and the lists under them). */
const without = (layout: Layout, gone: readonly string[]): Layout => ({
  at: new Map([...layout.at].filter(([key]) => !gone.includes(key))),
  kids: new Map([...layout.kids].filter(([parent]) => parent === null || !gone.includes(parent)).map(([parent, keys]) => [parent, keys.filter((key) => !gone.includes(key))] as const)),
});
/** A link as text (every member, sorted) and the ids of its two ends, which the links of a dropped row are found by. */
type LinkText = readonly [text: string, from: string, to: string];
const linkTexts = (links: readonly PlainLink[]): LinkText[] => links.map((link) => [JSON.stringify(Object.entries(link).sort(([a], [b]) => (a < b ? -1 : 1))), String(link.from), String(link.to)]);
/** The links that touch none of the rows with the ids in `gone`: a dropped row's links go with it. */
const staying = (links: readonly LinkText[], gone: ReadonlySet<string>): LinkText[] => links.filter(([, from, to]) => !gone.has(from) && !gone.has(to));
/** A link of the data as the chart matches it: `id` is its `key`, else its two ends as task keys; `change` is what the chart is told of it. */
interface Matched { id: string; entry: LinkText; change: LinkChange }
const keysById = (layout: Layout): Map<string, string> => new Map([...layout.at].map(([key, at]) => [String((JSON.parse(at) as [unknown])[0]), key]));
/** The links of `list` that join two rows of `within`, their ends read as ids of `layout`. */
const matched = (list: readonly LinkText[], layout: Layout, within: Layout): Matched[] => {
  const keys = keysById(layout);
  return list.flatMap((entry) => {
    const { key, type, lag } = Object.fromEntries(JSON.parse(entry[0]) as [string, unknown][]) as PlainLink;
    const [pred, succ] = [keys.get(entry[1]), keys.get(entry[2])];
    if (pred === undefined || succ === undefined || !within.at.has(pred) || !within.at.has(succ)) return [];
    return [{ id: key === undefined ? JSON.stringify([pred, succ]) : `key ${key}`, entry, change: { pred, succ, ...(key === undefined ? {} : { key }), ...(type === undefined ? {} : { type }), ...(lag === undefined ? {} : { lag }) } }];
  });
};
/** The links to add and drop to make `held` the links of `next`, or why `next` is more: a key or pair listed twice, or a link that stays (by its key, else its two ends) with another member. */
function linksOf(held: readonly Matched[], next: readonly Matched[]): string | { adds: Matched[]; drops: Matched[] } {
  const text = (list: readonly Matched[]) => list.map(({ entry }) => entry[0]).sort();
  if (same(text(next), text(held))) return { adds: [], drops: [] };
  const was = new Map(held.map((link) => [link.id, link]));
  const now = new Set(next.map(({ id }) => id));
  if (now.size !== next.length) return "a key or a pair of tasks is listed twice among the links.";
  if (next.some((link) => was.has(link.id) && was.get(link.id)!.entry[0] !== link.entry[0])) return "a link that stays has another member (its type, lag or ends).";
  return { adds: next.filter(({ id }) => !was.has(id)), drops: held.filter(({ id }) => !now.has(id)) };
}

const setOf = (values: readonly unknown[], was?: readonly unknown[]) => MEMBERS.flatMap((member, i) => (values[i] !== undefined && (was === undefined || differs(values[i], was[i])) ? [[member, values[i]] as const] : []));
/** `set` with the row's `actualStart` and `actualFinish` (those it holds) put beside a changed `progress` when the row holds a start: the chart then reads the percent beside those dates, as `mount` does, where a lone `progress` is a typed percent and drops a finish. */
const beside = (set: ReturnType<typeof setOf>, values: readonly unknown[]) => {
  const held = (member: "actualStart" | "actualFinish") => values[MEMBERS.indexOf(member)];
  if (!set.some(([member]) => member === "progress") || typeof held("actualStart") !== "string") return set;
  return [...set, ...(["actualStart", "actualFinish"] as const).filter((member) => typeof held(member) === "string" && !set.some(([name]) => name === member)).map((member) => [member, held(member)] as const)];
};
const idsIn = (layout: Layout, keys: readonly string[]) => new Set(keys.map((key) => String((JSON.parse(layout.at.get(key)!) as [unknown])[0])));
/** The refusal for the first task that gives a date limit in two spellings, or undefined. */
function twiceRefusal(tasks: readonly PlainTask[]): UpdateResult | undefined {
  for (const task of tasks) {
    const twice = spelledTwice(task);
    if (twice !== undefined) return Object.freeze({ ok: false, reason: "badRecords", message: `Nothing changed: task ${task.key} gives both "constraint" and "${twice}", two spellings of one date limit; give one.` });
  }
  return undefined;
}
/** The rows held as the chart holds them: those of `ghosts` it holds no live task of (`absent`: rows a person deleted and has not brought back) left out of the members, the layout and the links, kept for when it does. */
function seen(ghosts: readonly string[], live: (keys: string[]) => boolean[], byKey: ReadonlyMap<string, readonly unknown[]>, layout: Layout, links: readonly LinkText[]) {
  const alive = live([...ghosts]);
  const absent = ghosts.filter((_, i) => !alive[i]);
  return { absent, old: new Map([...byKey].filter(([key]) => !absent.includes(key))), held: without(layout, absent), links: staying(links, idsIn(layout, absent)) };
}

/** The compared members of each task by its `key`, or `undefined` when a task has no text `key` or two share one. */
function membersByKey(tasks: readonly PlainTask[]): Map<string, readonly unknown[]> | undefined {
  const byKey = new Map<string, readonly unknown[]>();
  for (const task of tasks) {
    if (typeof task.key !== "string" || byKey.has(task.key)) return undefined;
    byKey.set(task.key, MEMBERS.map((member) => task[member]));
  }
  return byKey;
}

/** True when a task of `next` has no text `key`, or differs from the task of `held` with its key in a member `setData` ignores (`color`, `end` and the rest, the host's own included), or is a task `held` lacks that gives one (an added task's `color` and `label` are set in place): the rows must start the chart again, or the change is dropped. A changed `id` or `parent` is `setData`'s to refuse. */
export function unsetChange(held: readonly PlainTask[], next: readonly PlainTask[]): boolean {
  const before = new Map(held.map((task) => [task.key, task]));
  const taken = new Set<string>([...MEMBERS, "key", "id", "parent"]);
  const added = new Set<string>([...taken, ...LOOK]);
  return next.some((task) => {
    const was = typeof task.key === "string" ? before.get(task.key) : undefined;
    return typeof task.key !== "string" || (was === undefined ? Object.keys(task).some((member) => !added.has(member) && task[member] !== undefined) : MEMBERS.some((member) => was[member] !== undefined && task[member] === undefined) || [...new Set([...Object.keys(was), ...Object.keys(task)])].some((member) => !taken.has(member) && differs(task[member], was[member])));
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
  let kept = { epoch, tasks: weak(first.tasks), byKey: laid && keyed, layout: laid, links: weak(first.links), linkTexts: keyed ? linkTexts(first.links ?? []) : [], day: first.dataDate?.slice(0, 10), ghosts: [] as string[] };
  return (next, now, apply, live = (keys) => keys.map(() => true)) => {
    const kept0 = kept.byKey;
    const full = kept.layout;
    if (kept0 === undefined || full === undefined) return rowsChanged("the chart holds no rows of yours to compare with (every task needs its own text key).");
    if (kept.epoch !== now) return rowsChanged("a schedule was opened since the rows were given.");
    const { absent, old, held, links } = seen(kept.ghosts, live, kept0, full, kept.linkTexts);
    const idsOf = (keys: readonly string[]) => idsIn(full, keys);
    const byKey = new Map(old);
    const records: TaskUpdate[] = [];
    let adds: Add[] = [];
    let drops: string[] = [];
    let layout = held;
    if (next.tasks !== undefined && next.tasks !== kept.tasks?.deref()) {
      const given = membersByKey(next.tasks);
      if (given === undefined) return rowsChanged("every task needs its own text key.");
      const twice = twiceRefusal(next.tasks);
      if (twice !== undefined) return twice;
      const to = layoutOf(next.tasks);
      if (to === undefined) return rowsChanged("a task's parent is no task of the rows.");
      const places = placesOf(held, to);
      if (typeof places === "string") return rowsChanged(places);
      layout = to;
      drops = dropsOf(held, to);
      const rowOf = new Map(next.tasks.map((task) => [task.key, task]));
      adds = places.map(({ key, ...place }) => ({ ...place, record: { key, ...Object.fromEntries(setOf(given.get(key)!)), ...lookOf(rowOf.get(key)!) } }));
      for (const [key, values] of given) {
        const was = old.get(key);
        const set = beside(setOf(values, was), values);
        if (was !== undefined && set.length > 0) records.push({ key, ...Object.fromEntries(set) });
        byKey.set(key, values.map((value, i) => (value === undefined ? was?.[i] : value)));
      }
    }
    const found = next.links !== undefined && next.links !== kept.links?.deref() ? linksOf(matched(links, held, layout), matched(linkTexts(next.links), layout, layout)) : { adds: [], drops: [] };
    if (typeof found === "string") return rowsChanged(found);
    const { adds: linkAdds, drops: linkDrops } = found;
    const day = next.dataDate === undefined ? undefined : dayOf(next.dataDate);
    if (next.dataDate !== undefined && day === undefined) return Object.freeze({ ok: false, reason: "badDate", message: "Nothing changed: write the data date as YYYY-MM-DD." });
    const move = day !== kept.day ? day : undefined;
    const idle = records.length === 0 && adds.length === 0 && drops.length === 0 && linkAdds.length === 0 && linkDrops.length === 0 && move === undefined;
    const { added, dropped, skipped = [], linked, unlinked, ...answer } = idle ? { ok: true as const, updated: 0 } : apply(records, move, adds, drops, { adds: linkAdds.map(({ change }) => change), drops: linkDrops.map(({ change }) => change) });
    const took = answer.ok ? adds.length : added;
    if (took !== undefined) { // the chart took the records: the rows it was given, the first `took` adds and the first drops are held, and what it refused is offered again
      const lost = answer.ok ? drops.length : dropped ?? 0;
      const taken = drops.slice(0, lost);
      adds.slice(took).forEach(({ record }) => byKey.delete(record.key));
      taken.filter((key) => !skipped.includes(key)).forEach((key) => byKey.delete(key));
      const out = absent.filter((key) => !byKey.has(key)); // the deleted rows still not the chart's: those the rows did not give, or whose add was refused
      const rows = took === adds.length ? layout : layoutOf((next.tasks ?? []).filter(({ key }) => byKey.has(key!)));
      out.forEach((key) => byKey.set(key, kept0.get(key)!));
      const ghosts = [...out, ...taken.filter((key) => skipped.includes(key))];
      const stays = staying(kept.linkTexts, idsOf([...taken.filter((key) => !skipped.includes(key)), ...absent.filter((key) => !out.includes(key))])).filter((entry) => !linkDrops.slice(0, answer.ok ? linkDrops.length : unlinked ?? 0).some((drop) => drop.entry === entry));
      kept = { ...kept, byKey, ghosts, layout: withRest(rows, full, [...drops.slice(lost), ...ghosts]), linkTexts: [...stays, ...linkAdds.slice(0, answer.ok ? linkAdds.length : linked ?? 0).map(({ entry }) => entry)], ...(answer.ok && next.tasks !== undefined && { tasks: weak(next.tasks) }), ...(answer.ok && next.links !== undefined && { links: weak(next.links) }), ...(move !== undefined && { day: move }) };
    }
    return Object.freeze(answer) as UpdateResult;
  };
}
