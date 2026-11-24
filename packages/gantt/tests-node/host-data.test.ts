// `setData` (decision 0022, rules 4.1 and 4.2): only the members that differ from the rows last given reach `update`, and a row or link that differs is refused. No engine is needed.
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { PlainLink, PlainTask } from "../src/adapters/plain.ts";
import type { TaskUpdate, UpdateResult } from "../src/gantt-types.ts";
import { hostData, unsetChange, type Add } from "../src/host-data.ts";

const task = (n: number, extra: object = {}): PlainTask => ({ id: n, key: `k${n}`, name: `Task ${n}`, start: "2026-03-02", duration: 2, progress: 0, ...extra });
const rows = (): PlainTask[] => [task(1), task(2), task(3)];
const links: PlainLink[] = [{ from: 1, to: 2 }, { from: 2, to: 3, lag: 1 }];

/** A chart that has been given `tasks`, `links` and a data date at epoch 0, the records its `update` was asked to set, the days its `setDataDate` was, and the rows its engine was asked to add: it takes `took` of them and refuses the next as "Partly applied" (all of them by default). */
function chart(answer: UpdateResult | ((records: TaskUpdate[]) => UpdateResult) = (records) => ({ ok: true, updated: records.length }), took = Infinity) {
  const calls: TaskUpdate[][] = [];
  const days: string[] = [];
  const added: Add[][] = [];
  const first = { tasks: rows(), links: links.map((link) => ({ ...link })), dataDate: "2026-03-02T08:00" };
  const set = hostData(first, 0);
  const apply = (records: TaskUpdate[], day: string | undefined, adds: Add[]): UpdateResult & { added?: number } => {
    if (records.length > 0) calls.push(records);
    if (day !== undefined) days.push(day);
    if (adds.length > 0) added.push(adds);
    const done = typeof answer === "function" ? answer(records) : answer;
    if (!done.ok) return done;
    return adds.length <= took ? { ok: true, updated: done.updated + adds.length } : { ok: false, reason: "badValue", message: "Partly applied: no", added: took };
  };
  return { calls, days, added, first, give: (data: { tasks?: PlainTask[]; links?: PlainLink[]; dataDate?: string }, epoch = 0) => set(data, epoch, apply) };
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

test("a task removed, moved or given another id or parent, a key that is no text, a repeated key, a parent no row has and a changed link are refused with nothing set", () => {
  const { calls, added, give } = chart();
  const refusals = [
    give({ tasks: rows().slice(1) }),
    give({ tasks: [task(1), task(2), task(9)] }),
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
  expect(refusals.map((answer) => (answer.ok ? "ok" : answer.reason))).toEqual(Array(14).fill("rowsChanged"));
  expect(refusals.every((answer) => !answer.ok && answer.message.startsWith("Nothing changed"))).toBe(true);
  expect([calls, added], "nothing was set or added").toEqual([[], []]);
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
  const { calls, added, give } = chart(undefined, 1);
  const next = [task(1), task(2, { duration: 9 }), task(3), task(4), task(5)];
  expect(give({ tasks: next })).toMatchObject({ ok: false, reason: "badValue", message: expect.stringContaining("Partly applied") });
  expect(calls, "the row that differs was set before the adds").toEqual([[{ key: "k2", duration: 9 }]]);
  expect(give({ tasks: next }), "the same array again: the rest is added and nothing is set twice").toEqual({ ok: true, updated: 1 });
  expect(added.map((adds) => adds.map(({ record: { key }, after }) => [key, after]))).toEqual([[["k4", "k3"], ["k5", "k4"]], [["k5", "k4"]]]);
  expect(calls).toEqual([[{ key: "k2", duration: 9 }]]);
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
  expect(unsetChange(held, [task(1, { name: "Renamed", progress: 50 }), task(2), task(3)]), "a name").toBe(false);
  expect(unsetChange(held, [task(1), task(2), task(3), task(4)]), "a new task with the members setData takes is left to it").toBe(false);
  expect(unsetChange(held, [task(1), task(2), task(3), task(4, { color: "red" })]), "a new task with a member setData would drop").toBe(true);
});
