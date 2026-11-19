// The typed constraint at mount (decision 0022, rule 9.5), in the flat form's partial pairs, and in setData: a row's constraint wins over its start, and two spellings of one limit are refused. No engine is needed.
import { fromTasks } from "../src/adapters/plain.ts";
import { validate } from "../src/adapters/validate.ts";
import { toSeparate } from "../src/constraint-form.ts";
import { readFields } from "../src/field-map.ts";
import { hostData } from "../src/host-data.ts";
import type { ConstraintType } from "../src/constraint-form.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const TYPES: ConstraintType[] = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo"];
const floorOf = (row: Record<string, unknown>) => fromTasks({ tasks: [{ id: 1, name: "A", start: "2026-03-02", duration: 2, ...row }] }).document.tasks[0]!;

test("at mount a row's constraint wins over its start: no floor from start for each type and for null, and the limit of the type is read", () => {
  for (const type of TYPES) {
    const typed = floorOf({ constraint: { type, date: "2026-03-16" } });
    const spelled = floorOf({ [type]: "2026-03-16" });
    if (type === "snet") expect(typed.snet, "a start-no-earlier-than is the floor").toEqual(spelled.snet);
    else {
      expect(typed.snet, `${type}: the start is no floor`).toBeNull();
      expect(Object.keys(typed.limit ?? {}), `${type}: the limit of the type is read (its date counts from the origin, which a start moves)`).toEqual(Object.keys(spelled.limit ?? {}));
      expect(spelled.snet, `${type}: the separate spelling keeps its start as the floor`).not.toBeNull();
    }
  }
  expect(floorOf({ constraint: null }).snet, "null is no floor").toBeNull();
  expect(floorOf({ constraint: null }).limit, "and no limit").toBeUndefined();
  expect(floorOf({ snlt: "2026-03-20" }).snet, "the separate spelling is unchanged: start is a floor").not.toBeNull();
});

test("a row with a constraint needs no start to validate, and still needs one without it; both spellings are a problem", () => {
  const task = { id: 1, name: "A" };
  expect(validate({ tasks: [{ ...task, constraint: { type: "fnlt", date: "2026-03-20" } }, { id: 2, name: "B", constraint: null }] })).toEqual({ ok: true });
  expect(validate({ tasks: [task] }).ok).toBe(false);
  const startless = (row: Record<string, unknown>) => fromTasks({ tasks: [{ ...task, duration: 2, ...row }] }).document.tasks[0]!;
  expect(startless({ constraint: { type: "snet", date: "2026-03-09" } }).snet, "the constraint is the floor, with no start").not.toBeNull();
  expect([startless({ constraint: { type: "fnlt", date: "2026-03-20" } }).snet, startless({ constraint: null }).snet], "a limit that is no floor, and null, leave none").toEqual([null, null]);
  const twice = validate({ tasks: [{ ...task, constraint: { type: "snet", date: "2026-03-09" }, fnet: "2026-03-20" }] });
  expect(twice.ok === false && twice.issues.map((issue) => issue.path)).toEqual(["tasks[0].fnet"]);
  expect(() => floorOf({ constraint: { type: "snet", date: "2026-03-09" }, fnet: "2026-03-20" })).toThrow(/constraint or this member/);
});

const flat = { tasks: { constraint: { type: "limitType", date: "limitDate" } } };
const names = readFields(flat, () => undefined);

test("the flat form: both members left out is no constraint, both null is null, and a partial pair is a constraint the chart refuses", () => {
  expect(names.tasks.toChart({ key: "a", name: "A", limitType: undefined, limitDate: undefined }), "left out: the task's limit is untouched").toEqual({ key: "a", name: "A" });
  expect(names.tasks.toChart({ key: "a", limitType: null, limitDate: null })).toEqual({ key: "a", constraint: null });
  for (const part of [{ limitType: null }, { limitDate: null }, { limitType: "snet", limitDate: null }, { limitType: null, limitDate: "2026-03-02" }, { limitType: undefined, limitDate: "2026-03-02" }]) {
    const out = names.tasks.toChart({ key: "a", ...part }) as Record<string, unknown>;
    expect(() => toSeparate(out), JSON.stringify(part)).toThrow(/is { type, date }/);
  }
  const taken = names.custom.take("tasks", [{ key: "a", id: "a", name: "A", start: "2026-03-02", limitType: undefined, limitDate: undefined, owner: "x" }]);
  expect(Object.keys(taken.rows![0]!).sort(), "the two members stay known chart fields").toEqual(["id", "key", "limitDate", "limitType", "name", "start"]);
});

test("setData refuses a task that gives constraint with any of the six members, whether or not that member changed", () => {
  const row = { id: 1, key: "k1", name: "A", start: "2026-03-02", duration: 2, progress: 0 };
  for (const type of TYPES) {
    const both = { ...row, constraint: { type: "snet" as const, date: "2026-03-02" }, [type]: "2026-03-20" };
    const answer = hostData({ tasks: [both] }, 0)({ tasks: [{ ...both }] }, 0, () => ({ ok: true, updated: 1 }));
    expect(answer, type).toMatchObject({ ok: false, reason: "badRecords", message: expect.stringContaining(`both "constraint" and "${type}"`) });
  }
});
