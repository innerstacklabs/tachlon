// `setData` (decision 0022, rules 4.1 and 4.2): only the members that differ from the rows last given reach `update`, and a row or link that differs is refused. No engine is needed.
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { PlainLink, PlainTask } from "../src/adapters/plain.ts";
import type { TaskUpdate, UpdateResult } from "../src/gantt-types.ts";
import { hostData } from "../src/host-data.ts";

const task = (n: number, extra: object = {}): PlainTask => ({ id: n, key: `k${n}`, name: `Task ${n}`, start: "2026-03-02", duration: 2, progress: 0, ...extra });
const rows = (): PlainTask[] => [task(1), task(2), task(3)];
const links: PlainLink[] = [{ from: 1, to: 2 }, { from: 2, to: 3, lag: 1 }];

/** A chart that has been given `tasks` and `links` at epoch 0, and the records its `update` was asked to set. */
function chart(answer: UpdateResult | ((records: TaskUpdate[]) => UpdateResult) = (records) => ({ ok: true, updated: records.length })) {
  const calls: TaskUpdate[][] = [];
  const first = { tasks: rows(), links: links.map((link) => ({ ...link })) };
  const set = hostData(first, 0);
  const apply = (records: TaskUpdate[]): UpdateResult => { calls.push(records); return typeof answer === "function" ? answer(records) : answer; };
  return { calls, first, give: (data: { tasks?: PlainTask[]; links?: PlainLink[] }, epoch = 0) => set(data, epoch, apply) };
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
  expect(calls, "update answers mso and mfo notUpdatable").toEqual([[{ key: "k1", ...limits }]]);
  expect(give({ tasks: [task(1, limits), task(2), task(3)] }), "the same limits again").toEqual({ ok: true, updated: 0 });
  give({ tasks: [task(1, { ...limits, fnlt: "2026-03-26" }), task(2), task(3)] });
  expect(calls[1], "only the limit that changed").toEqual([{ key: "k1", fnlt: "2026-03-26" }]);
});

test("a task added, removed, moved or given another id or parent, a key that is no text, a repeated key and a changed link are refused with nothing set", () => {
  const { calls, give } = chart();
  const refusals = [
    give({ tasks: [...rows(), task(4)] }),
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
  ];
  expect(refusals.map((answer) => (answer.ok ? "ok" : answer.reason))).toEqual(Array(11).fill("rowsChanged"));
  expect(refusals.every((answer) => !answer.ok && answer.message.startsWith("Nothing changed"))).toBe(true);
  expect(calls).toEqual([]);
});

test("links that differ only in the order of their members are the same links", () => {
  const { give } = chart();
  expect(give({ links: [{ to: 2, from: 1 }, { lag: 1, to: 3, from: 2 }] })).toEqual({ ok: true, updated: 0 });
});

test("a refusal of update leaves the rows as they were: the same change is offered again", () => {
  let refuse = true;
  const { calls, give } = chart((records) => (refuse ? { ok: false, reason: "badValue", message: "no" } : { ok: true, updated: records.length }));
  const next = [task(1, { duration: 5 }), task(2), task(3)];
  expect(give({ tasks: next }).ok).toBe(false);
  refuse = false;
  expect(give({ tasks: next })).toEqual({ ok: true, updated: 1 });
  expect(calls).toEqual([[{ key: "k1", duration: 5 }], [{ key: "k1", duration: 5 }]]);
});

test("a chart with no rows of the host's, or a schedule opened since, refuses", () => {
  const empty = hostData({}, 0);
  const apply = (): UpdateResult => { throw new Error("nothing is set"); };
  expect(empty({ tasks: rows() }, 0, apply)).toMatchObject({ ok: false, reason: "rowsChanged" });
  const { give } = chart();
  expect(give({ tasks: rows() }, 1)).toMatchObject({ ok: false, reason: "rowsChanged" });
});
