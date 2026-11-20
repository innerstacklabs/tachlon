// The typed constraint at the boundary (decision 0022, rule 9): one `constraint`, `{ type, date }` or `null`, in host data, updates and changes. No engine is needed.
import { foldConstraint, toSeparate, toTyped } from "../src/constraint-form.ts";
import { readFields } from "../src/field-map.ts";
import { hostData } from "../src/host-data.ts";
import type { Change } from "../src/events.js";
import type { ConstraintType } from "../src/constraint-form.ts";
import type { TaskUpdate } from "../src/gantt-types.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const TYPES: ConstraintType[] = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo"];
const update = (set: Record<string, unknown>, was: Record<string, unknown> = {}): Change => ({ record: "task", op: "update", key: "k", task: 0, set, was });

test("each of the six types is written as the member it names, and a change's member reads back as that type", () => {
  for (const type of TYPES) {
    expect(toSeparate({ key: "k", constraint: { type, date: "2026-03-09" } })).toEqual({ key: "k", [type]: "2026-03-09" });
    expect(foldConstraint(update({ [type]: "2026-03-09" }, { [type]: null }))).toEqual(update({ constraint: { type, date: "2026-03-09" } }, { constraint: null }));
  }
});

test("null is no limit: nothing on a row at mount, the four updatable members cleared in an update, and a change whose limits are all null", () => {
  expect(toSeparate({ key: "k", constraint: null })).toEqual({ key: "k" });
  expect(toSeparate({ key: "k", constraint: null }, true)).toEqual({ key: "k", snet: null, snlt: null, fnet: null, fnlt: null });
  expect(foldConstraint(update({ snet: null, fnlt: null }, { fnlt: "2026-03-09" }))).toEqual(update({ constraint: null }, { constraint: { type: "fnlt", date: "2026-03-09" } }));
});

test("in an update a limit replaces every bound: the other three are cleared, and a pin is its own member, which update reads against the pin the task holds", () => {
  expect(toSeparate({ key: "k", constraint: { type: "fnlt", date: "2026-03-09" } }, true)).toEqual({ key: "k", snet: null, snlt: null, fnet: null, fnlt: "2026-03-09" });
  expect(toSeparate({ key: "k", constraint: { type: "mso", date: "2026-03-09" } }, true)).toEqual({ key: "k", mso: "2026-03-09" });
});

test("a task with both spellings, or a constraint that is not { type, date } or null, is refused", () => {
  expect(() => toSeparate({ key: "k", constraint: { type: "snet", date: "2026-03-09" }, snlt: "2026-03-20" })).toThrow(/both "constraint" and "snlt"/);
  expect(() => toSeparate({ key: "k", constraint: null, mfo: "2026-03-20" })).toThrow(/both "constraint" and "mfo"/);
  for (const constraint of [{ type: "alap", date: "2026-03-09" }, { type: "snet" }, "snet", 3, []]) expect(() => toSeparate({ key: "k", constraint })).toThrow(/is { type, date }/);
  expect(toSeparate({ key: "k", snet: "2026-03-09" }), "a task with no constraint is returned as it is").toEqual({ key: "k", snet: "2026-03-09" });
});

test("a task with three bounds reads as the first of mso, mfo, snlt, fnlt, snet, fnet, and the other two are counted", () => {
  expect(toTyped({ snet: "2026-03-02", fnlt: "2026-03-20", snlt: "2026-03-09" })).toEqual({ constraint: { type: "snlt", date: "2026-03-09" }, leftOut: 2 });
  expect(toTyped({ fnet: "2026-03-30", mso: "2026-03-04", snet: "2026-03-02" })).toEqual({ constraint: { type: "mso", date: "2026-03-04" }, leftOut: 2 });
  expect(toTyped({ snet: null, fnet: "2026-03-30" })).toEqual({ constraint: { type: "fnet", date: "2026-03-30" }, leftOut: 0 });
  expect(toTyped({ name: "Pour" }), "members that name no limit say nothing").toEqual({ constraint: undefined, leftOut: 0 });
  const folded = foldConstraint(update({ snet: "2026-03-02", fnlt: "2026-03-20", name: "Pour" }));
  expect(folded.set).toEqual({ name: "Pour", constraint: { type: "fnlt", date: "2026-03-20" } });
});

test("in an update a start beside a constraint is dropped, since the constraint wins; a row at mount keeps the start the reader requires", () => {
  expect(toSeparate({ key: "k", start: "2026-03-02", constraint: { type: "fnet", date: "2026-03-30" } }, true)).toEqual({ key: "k", snet: null, snlt: null, fnlt: null, fnet: "2026-03-30" });
  expect(toSeparate({ key: "k", start: "2026-03-02", constraint: null }, true)).toEqual({ key: "k", snet: null, snlt: null, fnet: null, fnlt: null });
  expect(toSeparate({ key: "k", start: "2026-03-02", constraint: { type: "fnet", date: "2026-03-30" } })).toEqual({ key: "k", start: "2026-03-02", fnet: "2026-03-30" });
  expect(toSeparate({ key: "k", start: "2026-03-02" }, true)).toEqual({ key: "k", start: "2026-03-02" });
});

test("a change that is not a task's, or a task change that names no limit, is not touched", () => {
  const link: Change = { record: "link", op: "update", key: "l", link: 0, set: { snet: "x" }, was: { snet: "y" } };
  expect(foldConstraint(link)).toBe(link);
  const rename = update({ name: "Pour 2" }, { name: "Pour" });
  expect(foldConstraint(rename)).toEqual(rename);
});

const flat = { tasks: { constraint: { type: "limitType", date: "limitDate" } } };
const quiet = (fields: Parameters<typeof readFields>[0], form?: "typed" | "separate") => readFields(fields, () => undefined, form);

test("the flat form reads two members as one constraint and writes both in set and was, both null for none", () => {
  const names = quiet(flat);
  expect(names.tasks.rows([{ id: "a", key: "a", name: "A", start: "2026-03-02", limitType: "fnlt", limitDate: "2026-03-20" }])[0]).toEqual({ id: "a", key: "a", name: "A", start: "2026-03-02", constraint: { type: "fnlt", date: "2026-03-20" } });
  expect(names.tasks.toChart({ key: "a", limitType: null, limitDate: null })).toEqual({ key: "a", constraint: null });
  expect(names.tasks.toChart({ key: "a", limitType: "snet" }), "one member alone is a constraint the chart refuses").toEqual({ key: "a", constraint: { type: "snet", date: undefined } });
  const out = names.changes([update({ fnlt: "2026-03-20" }, { snet: "2026-03-02" })])[0]!;
  expect([out.set, out.was]).toEqual([{ limitType: "fnlt", limitDate: "2026-03-20" }, { limitType: "snet", limitDate: "2026-03-02" }]);
  expect(names.changes([update({ snet: null }, { snet: null })])[0]).toMatchObject({ set: { limitType: null, limitDate: null }, was: { limitType: null, limitDate: null } });
});

test("a one-member entry renames constraint, and the form is typed unless the host says separate", () => {
  const names = quiet({ tasks: { constraint: "limit" } });
  expect(names.tasks.toChart({ key: "a", limit: { type: "mfo", date: "2026-03-20" } })).toEqual({ key: "a", constraint: { type: "mfo", date: "2026-03-20" } });
  expect(names.changes([update({ mfo: "2026-03-20" })])[0]!.set).toEqual({ limit: { type: "mfo", date: "2026-03-20" } });
  const separate = quiet(undefined, "separate");
  expect(separate.changes([update({ mfo: "2026-03-20" })])[0]!.set, "under separate the six members are written as they are").toEqual({ mfo: "2026-03-20" });
  expect(quiet(undefined).changes([update({ mfo: "2026-03-20" })])[0]!.set).toEqual({ constraint: { type: "mfo", date: "2026-03-20" } });
});

test("an entry for constraint is refused under separate, and the flat form's two names are checked", () => {
  expect(() => quiet({ tasks: { constraint: "limit" } }, "separate")).toThrow(/"constraint".*constraintForm "typed"/);
  expect(() => quiet(flat, "separate")).toThrow(/constraintForm "typed"/);
  expect(() => quiet({ tasks: { constraint: { type: "limitType", date: "limitType" } } })).toThrow(/two different names/);
  expect(() => quiet({ tasks: { name: "limitType", constraint: flat.tasks.constraint } })).toThrow(/"limitType" to both "name" and "constraint"/);
  expect(() => quiet({ tasks: { constraint: { type: "start", date: "limitDate" } } })).toThrow(/"start", the chart's own name for another field/);
});

test("a constraint is not a custom field, with a map or without one, and the flat members are not either", () => {
  for (const [fields, row] of [[undefined, { constraint: null }], [{ tasks: { constraint: "limit" } }, { limit: null }], [flat, { limitType: "snet", limitDate: "2026-03-02" }]] as const) {
    const taken = quiet(fields).custom.take("tasks", [{ key: "a", id: "a", name: "A", start: "2026-03-02", owner: "sam", ...row }]);
    expect(Object.keys(taken.rows![0]!).sort(), JSON.stringify(fields)).toEqual(["id", "key", "name", ...Object.keys(row), "start"].sort());
  }
});

test("setData sets a constraint that differs by value, and nothing when an equal object comes again", () => {
  const first = [{ id: 1, key: "k1", name: "A", start: "2026-03-02", duration: 2, progress: 0, constraint: { type: "snet" as const, date: "2026-03-02" } }];
  const calls: TaskUpdate[][] = [];
  const give = hostData({ tasks: first }, 0);
  const apply = (records: TaskUpdate[]) => { calls.push(records); return { ok: true as const, updated: records.length }; };
  expect(give({ tasks: [{ ...first[0]!, constraint: { type: "snet", date: "2026-03-02" } }] }, 0, apply)).toEqual({ ok: true, updated: 0 });
  expect(give({ tasks: [{ ...first[0]!, constraint: { type: "fnlt", date: "2026-03-20" } }] }, 0, apply)).toEqual({ ok: true, updated: 1 });
  expect(give({ tasks: [{ ...first[0]!, constraint: null }] }, 0, apply)).toEqual({ ok: true, updated: 1 });
  expect(calls).toEqual([[{ key: "k1", constraint: { type: "fnlt", date: "2026-03-20" } }], [{ key: "k1", constraint: null }]]);
});
