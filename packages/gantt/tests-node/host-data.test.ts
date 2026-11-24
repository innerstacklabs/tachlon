// `setData` (decision 0022, rules 4.1 and 4.2): only the members that differ from the rows last given reach `update`, and a row or link that differs is refused. No engine is needed.
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { PlainLink, PlainTask } from "../src/adapters/plain.ts";
import type { TaskUpdate, UpdateResult } from "../src/gantt-types.ts";
import { hostData, unsetChange, type Add } from "../src/host-data.ts";

const task = (n: number, extra: object = {}): PlainTask => ({ id: n, key: `k${n}`, name: `Task ${n}`, start: "2026-03-02", duration: 2, progress: 0, ...extra });
const rows = (): PlainTask[] => [task(1), task(2), task(3)];
const links: PlainLink[] = [{ from: 1, to: 2 }, { from: 2, to: 3, lag: 1 }];

/** A chart that has been given `tasks`, `links` and a data date at epoch 0, the records its `update` was asked to set, the days its `setDataDate` was, the rows its engine was asked to add and the keys it was asked to drop: it takes `took` of the adds and `tookDrops` of the drops, and refuses the next as "Partly applied" (all of them by default). */
function chart(answer: UpdateResult | ((records: TaskUpdate[]) => UpdateResult) = (records) => ({ ok: true, updated: records.length }), took = Infinity, tookDrops = Infinity, start: PlainTask[] = rows()) {
  const out = new Set<string>(); // the keys of rows a person deleted: the chart holds no live task of them, and `apply` skips their drops
  const calls: TaskUpdate[][] = [];
  const days: string[] = [];
  const added: Add[][] = [];
  const dropped: string[][] = [];
  const first = { tasks: start, links: links.map((link) => ({ ...link })), dataDate: "2026-03-02T08:00" };
  const set = hostData(first, 0);
  const apply = (records: TaskUpdate[], day: string | undefined, adds: Add[], drops: string[]): UpdateResult & { added?: number; dropped?: number; skipped?: string[] } => {
    if (records.length > 0) calls.push(records);
    if (day !== undefined) days.push(day);
    if (adds.length > 0) added.push(adds);
    if (drops.length > 0) dropped.push(drops);
    const done = typeof answer === "function" ? answer(records) : answer;
    if (!done.ok) return done;
    if (adds.length > took) return { ok: false, reason: "badValue", message: "Partly applied: no", added: took };
    const skipped = drops.slice(0, tookDrops).filter((key) => out.has(key));
    return drops.length <= tookDrops ? { ok: true, updated: done.updated + adds.length, skipped } : { ok: false, reason: "hasChildren", message: "Partly applied: no", added: adds.length, dropped: tookDrops, skipped };
  };
  return { calls, days, added, dropped, first, out, give: (data: { tasks?: PlainTask[]; links?: PlainLink[]; dataDate?: string }, epoch = 0) => set(data, epoch, apply, (keys) => keys.map((key) => !out.has(key))) };
}

test("only the member that differs reaches update, with the task's key", () => {
  const { calls, give } = chart();
  expect(give({ tasks: [task(1), task(2, { duration: 5 }), task(3, { name: "Renamed", progress: 40 })] })).toEqual({ ok: true, updated: 2 });
  expect(calls).toEqual([[{ key: "k2", duration: 5 }, { key: "k3", name: "Renamed", progress: 40 }]]);
});

test("rows equal to the last given set nothing, and the same array is not read again", () => {
  const { calls, first, give } = chart();
  expect(give({ tasks: rows(), links: links.map((link) => ({ ...link })) })).toEqual({ ok: true, updated: 0 });
  expect(give({ tasks: first.tasks, links: first.links })).toEqual({ ok: true, updated: 0 });
  first.tasks[0]!.duration = 9; // changed in place and given again: the same array is skipped
  expect(give({ tasks: first.tasks })).toEqual({ ok: true, updated: 0 });
  expect(calls).toEqual([]);
});

test("a member left out keeps its last value, and a value that returns after one that was accepted is not new", () => {
  const { calls, give } = chart();
  give({ tasks: [task(1, { duration: 5 }), task(2), task(3)] });
  give({ tasks: [task(1, { duration: undefined }), task(2), task(3)] });
  give({ tasks: [task(1, { duration: 5 }), task(2), task(3)] });
  expect(calls, "the left-out member is not read as a change").toEqual([[{ key: "k1", duration: 5 }]]);
  give({ tasks: rows() });
  expect(calls[1], "a value that differs from the last accepted one is new").toEqual([{ key: "k1", duration: 2 }]);
});

test("a null start or snet is a value, and members that are not compared are ignored", () => {
  const { calls, give } = chart();
  give({ tasks: [task(1, { snet: "2026-03-09" }), task(2, { color: "#ff0000", milestone: "start" }), task(3)] });
  give({ tasks: [task(1, { snet: null }), task(2, { color: "#ff0000", milestone: "start" }), task(3)] });
  expect(calls).toEqual([[{ key: "k1", snet: "2026-03-09" }], [{ key: "k1", snet: null }]]);
});

test("a changed snlt, fnet, fnlt, mso or mfo reaches update by key, as update takes it, and one that did not change does not", () => {
  const { calls, give } = chart();
  const limits = { snlt: "2026-03-20", fnet: "2026-03-10", fnlt: "2026-03-25", mso: "2026-03-03", mfo: "2026-03-04" };
  expect(give({ tasks: [task(1, limits), task(2), task(3)] })).toEqual({ ok: true, updated: 1 });
  expect(calls, "mso and mfo are sent as they are").toEqual([[{ key: "k1", ...limits }]]);
  expect(give({ tasks: [task(1, limits), task(2), task(3)] }), "the same limits again").toEqual({ ok: true, updated: 0 });
  give({ tasks: [task(1, { ...limits, fnlt: "2026-03-26" }), task(2), task(3)] });
  expect(calls[1], "only the limit that changed").toEqual([{ key: "k1", fnlt: "2026-03-26" }]);
});

test("a task moved or given another id or parent, a key that is no text, a repeated key, a parent no row has and a changed link are refused with nothing set", () => {
  const { calls, added, dropped, give } = chart();
  const refusals = [
    give({ tasks: [task(1), task(2), task(3, { key: undefined })] }),
    give({ tasks: [task(1), task(1), task(3)] }),
    give({ links: [links[0]!] }),
    give({ tasks: [task(1, { duration: 7 }), task(2), task(3)], links: [...links, { from: 1, to: 3 }] }),
    give({ links: [links[0]!, { from: 2, to: 3, lag: 2 }] }),
    give({ tasks: [task(1), task(2, { parent: 1 }), task(3)] }),
    give({ tasks: [task(1), task(2, { id: 22 }), task(3)] }),
    give({ tasks: [task(2), task(1), task(3)] }),
    give({ tasks: [task(2), task(1), task(3), task(4)] }),
    give({ tasks: [task(1), task(2, { parent: 4 }), task(3), task(4)] }),
    give({ tasks: [task(1), task(2), task(3), task(4, { parent: 99 })] }),
    give({ tasks: [task(1), task(2), task(3), task(4, { parent: 5 }), task(5, { parent: 4 })] }),
  ];
  expect(refusals.map((answer) => (answer.ok ? "ok" : answer.reason))).toEqual(Array(12).fill("rowsChanged"));
  expect(refusals.every((answer) => !answer.ok && answer.message.startsWith("Nothing changed"))).toBe(true);
  expect([calls, added, dropped], "nothing was set, added or dropped").toEqual([[], [], []]);
});

const record = (n: number, extra: object = {}) => ({ key: `k${n}`, name: `Task ${n}`, start: "2026-03-02", duration: 2, progress: 0, ...extra });

test("a task the data holds and the chart does not is added after its previous sibling, first when it has none, and the rows are then held", () => {
  const { calls, added, give } = chart();
  expect(give({ tasks: [task(1), task(2), task(3), task(4)] })).toEqual({ ok: true, updated: 1 });
  expect(give({ tasks: [task(1), task(5), task(2), task(3), task(4)] })).toEqual({ ok: true, updated: 1 });
  expect(give({ tasks: [task(6), task(1), task(5), task(2), task(3), task(4)] })).toEqual({ ok: true, updated: 1 });
  expect(added.map((adds) => adds.map(({ record: { key }, parent, after }) => [key, parent, after])), "the key, the parent's key and the previous sibling's key, none for none").toEqual([[["k4", undefined, "k3"]], [["k5", undefined, "k1"]], [["k6", undefined, undefined]]]);
  expect(added[0]![0]!.record, "the record is the key and the members the task gives").toEqual(record(4));
  expect(give({ tasks: [task(6), task(1), task(5), task(2), task(3), task(4)] }), "the rows given again").toEqual({ ok: true, updated: 0 });
  expect(calls, "no row of the chart was set").toEqual([]);
});

test("a task added under a row that is held or that is added with it goes in outline order, whatever order the data lists it in, and takes the members update takes", () => {
  const { added, give } = chart();
  const limits = { actualStart: "2026-03-02", actualFinish: "2026-03-03", remaining: 1, constraint: { type: "snet", date: "2026-03-09" } };
  const answer = give({ tasks: [task(1), task(5, { parent: 4, ...limits }), task(2, { parent: undefined }), task(4, { parent: 2 }), task(3), task(6, { parent: 1 }), task(7, { parent: 4 })] });
  expect(answer).toEqual({ ok: true, updated: 4 });
  expect(added[0]!.map(({ record: { key }, parent, after }) => [key, parent, after]), "a parent before its children; the first row under a parent has no previous sibling").toEqual([["k6", "k1", undefined], ["k4", "k2", undefined], ["k5", "k4", undefined], ["k7", "k4", "k5"]]);
  expect(added[0]![2]!.record, "the actual dates, the remaining time and the limit reach the engine").toEqual(record(5, limits));
});

test("a refusal part way keeps the rows added before it, and the same rows given again add the rest", () => {
  const { calls, days, added, give } = chart(undefined, 1);
  const next = [task(1), task(2, { duration: 9 }), task(3), task(4), task(5)];
  expect(give({ tasks: next, dataDate: "2026-03-09" })).toMatchObject({ ok: false, reason: "badValue", message: expect.stringContaining("Partly applied") });
  expect(calls, "the row that differs was set before the adds").toEqual([[{ key: "k2", duration: 9 }]]);
  expect(give({ tasks: next, dataDate: "2026-03-09" }), "the same array again: the rest is added and nothing is set twice").toEqual({ ok: true, updated: 1 });
  expect(added.map((adds) => adds.map(({ record: { key }, after }) => [key, after]))).toEqual([[["k4", "k3"], ["k5", "k4"]], [["k5", "k4"]]]);
  expect([calls, days], "the data date moved once: it was held when the chart took the records").toEqual([[[{ key: "k2", duration: 9 }]], ["2026-03-09"]]);
});

test("a task the chart holds and the data lacks is dropped, the deepest first, with the records set and the adds made, and the rows are then held", () => {
  const { calls, added, dropped, give } = chart();
  expect(give({ tasks: [task(1, { duration: 9 }), task(2), task(4)] })).toEqual({ ok: true, updated: 2 });
  expect([calls, added.map((adds) => adds.map(({ record }) => record.key)), dropped], "the record and the add go with the drop of k3").toEqual([[[{ key: "k1", duration: 9 }]], [["k4"]], [["k3"]]]);
  expect(give({ tasks: [task(1, { duration: 9 }), task(2), task(4)] }), "the rows given again").toEqual({ ok: true, updated: 0 });
  const tree = [task(1), task(2, { parent: 1 }), task(3, { parent: 2 }), task(4), task(5, { parent: 4 })];
  const deep: string[][] = [];
  const done = (_records: TaskUpdate[], _day: string | undefined, _adds: Add[], drops: string[]): UpdateResult => { deep.push(drops); return { ok: true, updated: 0 }; };
  expect(hostData({ tasks: tree }, 0)({ tasks: [task(6)] }, 0, done)).toEqual({ ok: true, updated: 0 });
  expect(deep, "a row's children go before it; rows of one depth keep the order they were held in").toEqual([["k3", "k2", "k5", "k1", "k4"]]);
});

test("a kept child of a dropped row is a move and is refused, with nothing dropped", () => {
  const apply = (): UpdateResult => { throw new Error("nothing is dropped"); };
  const set = hostData({ tasks: [task(1), task(2, { parent: 1 }), task(3)] }, 0);
  expect(set({ tasks: [task(2), task(3)] }, 0, apply), "the child now at the top level").toMatchObject({ ok: false, reason: "rowsChanged" });
  expect(set({ tasks: [task(2, { parent: 1 }), task(3)] }, 0, apply), "the child under a row the rows lack").toMatchObject({ ok: false, reason: "rowsChanged" });
});

test("a link that touches a dropped row is no link change, and the links held are the ones that stay", () => {
  const first = chart();
  expect(first.give({ tasks: [task(1), task(2)] }), "no links given: the link to k3 went with it").toEqual({ ok: true, updated: 0 });
  expect(first.give({ links: [links[0]!] }), "the links held are the one that stays").toEqual({ ok: true, updated: 0 });
  expect(first.give({ links }), "a link to the dropped row is added back").toMatchObject({ ok: false, reason: "rowsChanged" });
  expect(chart().give({ tasks: [task(1), task(2)], links: [links[0]!] }), "the data drops the link with the row").toEqual({ ok: true, updated: 0 });
  const listed = chart();
  expect(listed.give({ tasks: [task(1), task(2)], links: links.map((link) => ({ ...link })) }), "the data still lists the link to the dropped row").toEqual({ ok: true, updated: 0 });
  const changed = chart();
  expect(changed.give({ tasks: [task(1), task(2)], links: [{ from: 1, to: 2, lag: 5 }] }), "a link between the rows that stay still differs").toMatchObject({ ok: false, reason: "rowsChanged" });
  expect(changed.dropped, "nothing was dropped").toEqual([]);
});

test("a drop the chart refuses part way keeps the drops before it, and the same rows given again drop the rest", () => {
  const { dropped, give } = chart(undefined, Infinity, 1);
  expect(give({ tasks: [task(1)] })).toMatchObject({ ok: false, reason: "hasChildren" });
  expect(give({ tasks: [task(1)] }), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(dropped).toEqual([["k2", "k3"], ["k3"]]);
});

test("links that differ only in the order of their members are the same links", () => {
  const { give } = chart();
  expect(give({ links: [{ to: 2, from: 1 }, { lag: 1, to: 3, from: 2 }] })).toEqual({ ok: true, updated: 0 });
});

test("a refusal of update leaves the rows as they were: the same change, and the add with it, are offered again", () => {
  let refuse = true;
  const { calls, added, give } = chart((records) => (refuse ? { ok: false, reason: "badValue", message: "no" } : { ok: true, updated: records.length }));
  const next = [task(1, { duration: 5 }), task(2), task(3), task(4)];
  expect(give({ tasks: next }).ok).toBe(false);
  refuse = false;
  expect(give({ tasks: next })).toEqual({ ok: true, updated: 2 });
  expect([calls, added.map((adds) => adds.length)]).toEqual([[[{ key: "k1", duration: 5 }], [{ key: "k1", duration: 5 }]], [1, 1]]);
});

test("a chart with no rows of the host's, or a schedule opened since, refuses", () => {
  const empty = hostData({}, 0);
  const apply = (): UpdateResult => { throw new Error("nothing is set"); };
  expect(empty({ tasks: rows() }, 0, apply)).toMatchObject({ ok: false, reason: "rowsChanged" });
  const { give } = chart();
  expect(give({ tasks: rows() }, 1)).toMatchObject({ ok: false, reason: "rowsChanged" });
});

test("rows without a key for every task read no task's parent and no link at the start, and setData still refuses them by name", () => {
  let reads = 0;
  const counted = <T extends object>(row: T, name: string): T => Object.defineProperty(row, name, { enumerable: true, get: () => { reads += 1; return 1; } });
  const set = hostData({ tasks: [counted(task(1), "parent"), counted(task(2, { key: undefined }), "parent")], links: [counted({ to: 2 }, "from") as PlainLink] }, 0);
  expect(reads, "the start reads no parent or link member").toBe(0);
  const apply = (): UpdateResult => { throw new Error("nothing is set"); };
  expect(set({ tasks: rows() }, 0, apply)).toEqual({ ok: false, reason: "rowsChanged", message: "Nothing changed: the chart holds no rows of yours to compare with (every task needs its own text key)." });
});

test("a data date that differs from the one given reaches the chart once, with the rows it came with, and the day it was given sets nothing", () => {
  const { calls, days, give } = chart();
  expect(give({ dataDate: "2026-03-02" }), "the day of the one mounted with, whose time of day is dropped").toEqual({ ok: true, updated: 0 });
  expect(give({ dataDate: "2026-03-09" })).toEqual({ ok: true, updated: 0 });
  expect(give({ dataDate: " 2026-03-09 " }), "the same day again, trimmed as setDataDate trims it").toEqual({ ok: true, updated: 0 });
  expect(days).toEqual(["2026-03-09"]);
  expect(give({ tasks: [task(1, { duration: 5 }), task(2), task(3)], dataDate: "2026-03-10" })).toEqual({ ok: true, updated: 1 });
  expect([calls, days]).toEqual([[[{ key: "k1", duration: 5 }]], ["2026-03-09", "2026-03-10"]]);
});

test("a data date that is not a real YYYY-MM-DD refuses the whole call by name and sets neither the rows nor the day", () => {
  const { calls, days, give } = chart();
  const next = [task(1, { duration: 5 }), task(2), task(3)];
  const bad = ["2026-02-30", "2026-3-9", "", "2026-03-09T08:00", "tomorrow", 20260309, null];
  expect(bad.map((dataDate) => give({ tasks: next, dataDate: dataDate as string }))).toEqual(bad.map(() => ({ ok: false, reason: "badDate", message: "Nothing changed: write the data date as YYYY-MM-DD." })));
  expect([calls, days]).toEqual([[], []]);
  expect(give({ tasks: next }), "the rows were not remembered").toEqual({ ok: true, updated: 1 });
});

test("a data date the chart refuses is offered again, not remembered", () => {
  let refuse = true;
  const { days, give } = chart(() => (refuse ? { ok: false, reason: "busy", message: "no" } : { ok: true, updated: 0 }));
  expect(give({ dataDate: "2026-03-09" }).ok).toBe(false);
  refuse = false;
  expect(give({ dataDate: "2026-03-09" })).toEqual({ ok: true, updated: 0 });
  expect(days).toEqual(["2026-03-09", "2026-03-09"]);
});

test("a changed remaining, actualStart or actualFinish reaches update by key and is no restart, and one left out that the task had is", () => {
  const started = { actualStart: "2026-03-02", actualFinish: "2026-03-03", remaining: 1 };
  const { calls, give } = chart();
  expect(give({ tasks: [task(1, started), task(2), task(3)] })).toEqual({ ok: true, updated: 1 });
  expect(give({ tasks: [task(1, { ...started, remaining: 3 }), task(2), task(3)] })).toEqual({ ok: true, updated: 1 });
  expect(calls).toEqual([[{ key: "k1", ...started }], [{ key: "k1", remaining: 3 }]]);
  expect(unsetChange([task(1, started)], [task(1, { ...started, remaining: 3 })]), "a new value is set in place").toBe(false);
  expect(unsetChange([task(1, started)], [task(1, { ...started, remaining: undefined })]), "a member left out").toBe(true);
});

test("unsetChange is true for a task that differs in a member setData ignores, or that has no key, and false for a name", () => {
  const held = rows();
  const ignored = { color: "red", label: "x", end: "2026-03-09", calendar: "c", milestone: "start", alap: true, resume: "2026-03-04", own: 1 };
  for (const [member, value] of Object.entries(ignored)) expect(unsetChange(held, [task(1), task(2, { [member]: value }), task(3)]), member).toBe(true);
  expect(unsetChange(held, [task(1), { id: 2, name: "No key" }, task(3)]), "no key").toBe(true);
  for (const member of ["start", "duration", "progress", "constraint"]) expect(unsetChange([task(1, { constraint: { type: "snet", date: "2026-03-02" } })], [{ ...task(1), [member]: undefined }]), `${member} left out`).toBe(true);
  expect(unsetChange(held, [task(1), task(2)]), "a task left out is a drop, not a restart").toBe(false);
  expect(unsetChange(held, [task(1, { name: "Renamed", progress: 50 }), task(2), task(3)]), "a name").toBe(false);
  expect(unsetChange(held, [task(1), task(2), task(3), task(4)]), "a new task with the members setData takes is left to it").toBe(false);
  expect(unsetChange(held, [task(1), task(2), task(3), task(4, { end: "2026-03-09" })]), "a new task with a member setData still ignores: the components mount the chart again").toBe(true);
});

test("an added task's color and label go in its record and are no restart, and a kept task's color or label that changed still is", () => {
  const { calls, added, give } = chart();
  const look = { color: "#FF00aa", label: "Shown" };
  expect(give({ tasks: [...rows(), task(4, look)] })).toEqual({ ok: true, updated: 1 });
  expect(added[0]![0]!.record, "the key, the members update takes and the look").toEqual({ key: "k4", name: "Task 4", start: "2026-03-02", duration: 2, progress: 0, ...look });
  expect(give({ tasks: [...rows(), task(4, { color: "#000000", label: "Other" })] }), "a kept task's look is no change setData sets").toEqual({ ok: true, updated: 0 });
  expect(calls).toEqual([]);
  expect(give({ tasks: [...rows(), task(4), task(5, { color: "red", label: 5 })] }), "a color that is not #rrggbb and a label that is not text are left out of the record, as mount leaves them out").toEqual({ ok: true, updated: 1 });
  expect(added[1]![0]!.record).toEqual({ key: "k5", name: "Task 5", start: "2026-03-02", duration: 2, progress: 0 });
  expect(unsetChange(rows(), [...rows(), task(4, { ...look, color: null })]), "an added task's look, null for none included").toBe(false);
  for (const change of [{ color: "#000000" }, { label: "Other" }, { color: undefined }]) expect(unsetChange([task(1, look)], [task(1, { ...look, ...change })]), JSON.stringify(change)).toBe(true);
});

test("a row the chart holds no live task of is held when its drop is skipped: once the chart holds it again the rows that give it set only what differs and add nothing, links included", () => {
  const { calls, added, dropped, out, give } = chart();
  out.add("k2");
  expect(give({ tasks: [task(1), task(3)] }), "k2 is a person's deleted row: skipped, counts nothing").toEqual({ ok: true, updated: 0 });
  out.delete("k2"); // Undo
  expect(give({ tasks: [task(1), task(2, { duration: 5 }), task(3)], links })).toEqual({ ok: true, updated: 1 });
  expect([calls, added, dropped]).toEqual([[[{ key: "k2", duration: 5 }]], [], [["k2"]]]);
});

test("a deleted row given again under another parent or in another place is refused as a move once Undo brought it back, and is an add while it is still deleted", () => {
  const { calls, added, dropped, out, give } = chart();
  out.add("k2");
  give({ tasks: [task(1), task(3)] });
  expect(give({ tasks: [task(1), task(2), task(3)] }), "still deleted: an add, after k1").toEqual({ ok: true, updated: 1 });
  expect(added[0]![0]).toMatchObject({ record: { key: "k2" }, after: "k1" });
  const again = chart();
  again.out.add("k2");
  again.give({ tasks: [task(1), task(3)] });
  again.out.delete("k2");
  expect(again.give({ tasks: [task(1), task(3), task(2)] }), "another place").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("moved") });
  expect(again.give({ tasks: [task(1), task(2, { parent: 1 }), task(3)] }), "another parent").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("another id or parent") });
  expect([again.calls, again.added, again.dropped], "nothing was set, added or dropped after the first drop").toEqual([[], [], [["k2"]]]);
  expect(give({ tasks: [task(1), task(2), task(3)] }), "k2 is held again as the chart's own row").toEqual({ ok: true, updated: 0 });
  expect([calls, dropped]).toEqual([[], [["k2"]]]);
});

test("a deleted row's children, dropped with it, are held with it: they come back in place with it, and a child the rows no longer give is dropped", () => {
  const nested = [task(1), task(2), task(3, { parent: 2 })];
  const { calls, added, dropped, out, give } = chart(undefined, undefined, undefined, nested);
  out.add("k2").add("k3");
  expect(give({ tasks: [task(1)] })).toEqual({ ok: true, updated: 0 });
  expect(dropped, "the child before its parent").toEqual([["k3", "k2"]]);
  out.clear();
  expect(give({ tasks: [task(1), task(2), task(3, { parent: 2, name: "Child" })] })).toEqual({ ok: true, updated: 1 });
  expect([calls, added]).toEqual([[[{ key: "k3", name: "Child" }]], []]);
  const lone = chart(undefined, undefined, undefined, nested);
  lone.out.add("k2").add("k3");
  lone.give({ tasks: [task(1)] });
  lone.out.clear();
  expect(lone.give({ tasks: [task(1), task(2)] }), "the child came back with its parent and the rows lack it").toEqual({ ok: true, updated: 0 });
  expect([lone.added, lone.dropped.at(-1)]).toEqual([[], ["k3"]]);
});
