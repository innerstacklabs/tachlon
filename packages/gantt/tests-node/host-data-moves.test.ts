// `setData` turns a task with another parent or place into an ordered list of moves (decision 0022, rule 4.1). No engine is needed.
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { PlainTask } from "../src/adapters/plain.ts";
import type { UpdateResult } from "../src/gantt-types.ts";
import { hostData, type Add, type Place } from "../src/host-data.ts";

const task = (n: number, extra: object = {}): PlainTask => ({ id: n, key: `k${n}`, name: `Task ${n}`, start: "2026-03-02", duration: 2, progress: 0, ...extra });
type Answer = UpdateResult & { added?: number; moved?: number };
/** A chart given `start`: `give` hands it rows and `calls` lists the adds, moves and drops its `apply` was asked for, which it answers with `answer`. */
function chart(start: PlainTask[], answer: (adds: Add[], moves: Place[]) => Answer = () => ({ ok: true, updated: 0 })) {
  const calls: { adds: Add[]; drops: string[]; moves: Place[] }[] = [];
  const set = hostData({ tasks: start }, 0);
  return { calls, give: (tasks: PlainTask[]) => set({ tasks }, 0, (_records, _day, adds, drops, _links, moves) => { calls.push({ adds, drops, moves }); return answer(adds, moves); }) };
}
const row = (key: string, parent?: string, after?: string): Place => ({ key, parent, after });

test("a reorder is the fewest moves: the rows that keep their order stay where they are", () => {
  const { calls, give } = chart([1, 2, 3, 4, 5].map((n) => task(n)));
  give([5, 1, 2, 3, 4].map((n) => task(n)));
  give([1, 2, 3, 4, 5].map((n) => task(n)));
  expect(calls.map(({ moves }) => moves), "k5 first, then k5 back after k4").toEqual([[row("k5")], [row("k5", undefined, "k4")]]);
  expect(give([1, 2, 3, 4, 5].map((n) => task(n))), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(calls).toHaveLength(2);
});

test("a row goes under a parent the same call adds: the add comes first, and the move names the new row", () => {
  const { calls, give } = chart([task(1), task(2), task(3)]);
  expect(give([task(4), task(1, { parent: 4 }), task(2), task(3)])).toEqual({ ok: true, updated: 0 });
  expect([calls[0]!.adds.map(({ record }) => record.key), calls[0]!.moves]).toEqual([["k4"], [row("k1", "k4")]]);
});

test("a refusal keeps the first moves taken, and the rows given again offer the rest", () => {
  const { calls, give } = chart([1, 2, 3, 4].map((n) => task(n)), (_adds, moves) => (moves.length > 2 ? { ok: false, reason: "badValue", message: "Partly applied: no", added: 0, moved: 1 } : { ok: true, updated: 0 }));
  const reversed = () => [4, 3, 2, 1].map((n) => task(n));
  expect(give(reversed())).toMatchObject({ ok: false });
  expect(give(reversed())).toEqual({ ok: true, updated: 0 });
  expect(calls.map(({ moves }) => moves), "k4 was taken; k3 after k4 and k2 after k3 are left").toEqual([[row("k4"), row("k3", undefined, "k4"), row("k2", undefined, "k3")], [row("k3", undefined, "k4"), row("k2", undefined, "k3")]]);
  const refused = chart([task(1), task(2)], (adds) => (adds.length > 0 ? { ok: false, reason: "badValue", message: "Partly applied: no", added: 0 } : { ok: true, updated: 0 }));
  const next = () => [task(2), task(1), task(3)];
  refused.give(next());
  refused.give(next());
  expect(refused.calls[1], "an add refused takes no move: both come again").toEqual(refused.calls[0]);
});

/** The rows of `tasks` as `key(children)` text, a row's children in the order given. */
const outline = (kids: Map<string | undefined, string[]>, parent?: string): string => (kids.get(parent) ?? []).map((key) => `${key}(${outline(kids, key)})`).join();
const kidsOf = (tasks: PlainTask[]) => tasks.reduce((kids, { key, parent }) => kids.set(parent === undefined ? undefined : `k${parent}`, [...(kids.get(parent === undefined ? undefined : `k${parent}`) ?? []), key!]), new Map<string | undefined, string[]>());

test("adds, then moves, then drops make the rows given from any rows held, each move to a place that exists", () => {
  let seed = 7;
  const random = (n: number): number => (seed = (seed * 48271) % 2147483647) % n;
  for (let round = 0; round < 300; round++) {
    const held: PlainTask[] = Array.from({ length: 1 + random(8) }, (_, i) => task(i + 1, i > 0 && random(2) === 0 ? { parent: 1 + random(i) } : {}));
    const ids = Array.from({ length: 10 }, (_, i) => i + 1).filter(() => random(4) > 0).sort(() => random(3) - 1);
    const next = ids.map((id, i) => task(id, i > 0 && random(2) === 0 ? { parent: ids[random(i)] } : {}));
    const { calls, give } = chart(held);
    expect(give(next), JSON.stringify([held, next])).toEqual({ ok: true, updated: 0 });
    const kids = kidsOf(held);
    const put = ({ key, parent, after }: Place): void => {
      kids.forEach((list) => { if (list.includes(key)) list.splice(list.indexOf(key), 1); });
      const list = kids.get(parent) ?? [];
      expect(after === undefined || list.includes(after), `${key} after ${after} under ${parent}`).toBe(true);
      list.splice(after === undefined ? 0 : list.indexOf(after) + 1, 0, key);
      kids.set(parent, list);
    };
    const [{ adds, moves, drops } = { adds: [], moves: [], drops: [] }] = calls;
    adds.forEach(({ record, parent, after }) => put({ key: record.key, parent, after }));
    moves.forEach(put);
    drops.forEach((key) => { expect(kids.get(key) ?? [], `${key} is empty when dropped`).toEqual([]); kids.forEach((list) => { if (list.includes(key)) list.splice(list.indexOf(key), 1); }); });
    expect(outline(kids), JSON.stringify([held, next])).toBe(outline(kidsOf(next)));
  }
});
