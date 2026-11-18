// One change per record (decision 0022, section 2, rules 2.1, 2.2, 2.6 and 2.7): the engine's per-field changes fold into `{ record, op, key, set, was }`.
// What the `edit` event and the before-edit function carry in the browser is `tests/events.spec.ts` and `tests/before-edit.spec.ts`.
import { type FieldChange, foldChanges } from "../src/change-set.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const field = (task: number, name: string, old: unknown, now: unknown, key: string | null = null): FieldChange => ({ record: "task", op: "update", key, task, field: name, old, new: now });
const link = (slot: number, op: "create" | "delete", from: unknown, to: unknown): FieldChange => ({ record: "link", op, key: null, link: slot, fields: { from, to, type: "FS", lag: 0 } });
const gone = (task: number, key: string | null, name: string): FieldChange => ({ record: "task", op: "delete", key, task, fields: { name, parent: null } });

test("three fields of one task are one change with three members in set and in was", () => {
  const changes = [field(0, "name", "Dig", "Dig deep", "k"), field(0, "duration", 1, 3, "k"), field(0, "snet", null, "2026-01-12T00:00", "k")];
  expect(foldChanges(changes)).toEqual([
    { record: "task", op: "update", key: "k", task: 0, set: { name: "Dig deep", duration: 3, snet: "2026-01-12T00:00" }, was: { name: "Dig", duration: 1, snet: null } },
  ]);
});

test("a record appears once, in the order it first appears, whatever the fields in between", () => {
  const changes = [field(2, "name", "a", "b"), field(0, "duration", 1, 2), field(2, "duration", 1, 4)];
  const folded = foldChanges(changes);
  expect(folded.map((c) => c.task)).toEqual([2, 0]);
  expect(folded[0]).toMatchObject({ set: { name: "b", duration: 4 }, was: { name: "a", duration: 1 } });
});

test("a create's set is the whole record and it has no was", () => {
  const fields = { parent: null, after: "a1", name: "New task", duration: 1 };
  const [change] = foldChanges([{ record: "task", op: "create", key: null, task: 3, fields }]);
  expect(change).toEqual({ record: "task", op: "create", key: null, task: 3, set: fields });
  expect("was" in change!).toBe(false);
});

test("a delete's was is the whole record, as it was before an update of the same commit", () => {
  const [change] = foldChanges([gone(1, "b2", "Dig"), field(1, "name", "Before", "Dig", "b2")]);
  expect(change).toEqual({ record: "task", op: "delete", key: "b2", task: 1, was: { name: "Before", parent: null } });
  expect("set" in change!).toBe(false);
});

test("a link delete beside the delete of one of its tasks is cascade, by key or by slot, and one beside nothing is not", () => {
  const byKey = foldChanges([link(0, "delete", "a1", "b2"), gone(1, "b2", "B")]);
  expect(byKey.map((c) => [c.record, c.op, c.cascade])).toEqual([["link", "delete", true], ["task", "delete", undefined]]);
  const bySlot = foldChanges([link(0, "delete", { task: 4 }, { task: 5 }), gone(5, null, "B")]);
  expect(bySlot[0]).toMatchObject({ record: "link", op: "delete", cascade: true, was: { from: { task: 4 }, to: { task: 5 }, type: "FS" } });
  const alone = foldChanges([link(0, "delete", "a1", "b2"), gone(2, "c3", "C")]);
  expect("cascade" in alone[0]!).toBe(false);
  expect("cascade" in foldChanges([link(0, "create", "a1", "b2"), gone(1, "b2", "B")])[0]!).toBe(false);
});

test("a child's delete stays before its parent's and the link between them is cascade", () => {
  const folded = foldChanges([gone(1, "b2", "Child"), gone(0, "a1", "Parent"), link(0, "delete", "a1", "b2")]);
  expect(folded.map((c) => c.key ?? c.link)).toEqual(["b2", "a1", 0]);
  expect(folded[2]!.cascade).toBe(true);
});

test("a schedule change has a null key and no slot", () => {
  const [change] = foldChanges([{ record: "schedule", op: "update", field: "dataDate", old: null, new: "2026-01-12T00:00" }]);
  expect(change).toEqual({ record: "schedule", op: "update", key: null, set: { dataDate: "2026-01-12T00:00" }, was: { dataDate: null } });
});
