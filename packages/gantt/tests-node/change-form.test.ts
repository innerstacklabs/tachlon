// A change in the unit and date form the host mounted with (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7). What an `edit` event and a before-edit call carry in a page is `tests/before-edit.spec.ts`.
import { changeForm, formChanges, readDays } from "../src/change-form.ts";
import type { PublicApp } from "../src/api.js";
import type { Change } from "../src/events.js";
import { test } from "node:test";
import { expect } from "@playwright/test";

// Tasks 0, 1 and 2 have days of 480, 450 and 450 minutes; link 0 counts a lag in 450, link 1 in 450 and link 2 in 1440.
let lengths = { tasks: [480, 450, 450], links: [450, 450, 1440] };
const reads: number[] = [];
const app = { day_lengths: () => (reads.push(1), JSON.stringify(lengths)) } as unknown as PublicApp;
const task = (slot: number, set: Record<string, unknown>, was: Record<string, unknown> = set, op: Change["op"] = "update"): Change => ({ record: "task", op, key: null, task: slot, set, was });
const link = (slot: number, lag: number, op: Change["op"] = "update"): Change => ({ record: "link", op, key: null, link: slot, ...(op === "delete" ? { was: { lag } } : { set: { lag }, was: { lag: 0 } }) });
const hours = (change: Change): unknown => formChanges([change], "hours", "minute", readDays(app)).map((c) => c.set ?? c.was)[0];

test("durations pass through in days", () => {
  const change = task(1, { duration: 2.5, remaining: 1 });
  expect(formChanges([change], "days", "minute", () => 1)).toEqual([change]);
  expect(formChanges([change], undefined, undefined, () => 1)).toEqual([change]);
});

test("hours and minutes on a 450-minute and a 480-minute calendar, in set and in was, for rows with no key", () => {
  const day = readDays(app);
  const [eight, seven] = [task(0, { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null }), task(1, { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null })];
  expect(formChanges([eight, seven], "hours", "minute", day)).toEqual([task(0, { duration: 16, remaining: 4 }, { duration: 8, remaining: null }), task(1, { duration: 15, remaining: 3.75 }, { duration: 7.5, remaining: null })]);
  expect(formChanges([eight, seven], "minutes", "minute", day).map((c) => c.set)).toEqual([{ duration: 960, remaining: 240 }, { duration: 900, remaining: 225 }]);
});

test("a lag is measured by its own link's day, and a row with no key has one", () => {
  expect(hours(link(0, 1))).toEqual({ lag: 7.5 });
  expect(hours(link(2, 1))).toEqual({ lag: 24 });
});

test("a task delete, the link delete that follows from it and the undo of both are measured as the rows were, though the rows are gone", () => {
  expect(hours(task(2, { duration: 1 }, { duration: 1 }, "delete"))).toEqual({ duration: 7.5 });
  expect(hours({ ...link(1, 2, "delete"), cascade: true })).toEqual({ lag: 15 });
  expect(hours(task(2, { duration: 1 }, {}, "create"))).toEqual({ duration: 7.5 });
  expect(hours(link(1, 2, "create"))).toEqual({ lag: 15 });
});

test("a slot the app does not hold takes the first task's day", () => {
  expect([hours(task(9, { duration: 1 })), hours(link(9, 1))]).toEqual([{ duration: 8 }, { lag: 8 }]);
});

test("dateForm day: a start-kind date is its day, a finish-kind date the day of its instant less one minute", () => {
  const dates = task(0, { snet: "2026-01-12T17:00", snlt: "2026-01-13T00:00", fnet: "2026-01-12T17:00", fnlt: "2026-01-13T00:00", actualStart: null, actualFinish: "2026-03-01T00:00", name: "T" }, { mfo: "2026-01-01T00:00", mso: "2026-01-01T00:00" });
  expect(formChanges([dates], "days", "day", () => 0)).toEqual([task(0, { snet: "2026-01-12", snlt: "2026-01-13", fnet: "2026-01-12", fnlt: "2026-01-12", actualStart: null, actualFinish: "2026-02-28", name: "T" }, { mfo: "2025-12-31", mso: "2026-01-01" })]);
  const schedule: Change = { record: "schedule", op: "update", key: null, set: { dataDate: "2026-01-12T00:00" }, was: { dataDate: "2026-01-09T08:00" } };
  expect(formChanges([schedule], "days", "day", () => 0)).toEqual([{ ...schedule, set: { dataDate: "2026-01-12" }, was: { dataDate: "2026-01-09" } }]);
  expect(formChanges([dates], "days", "minute", () => 0), 'dateForm "minute" leaves a date as the engine wrote it').toEqual([dates]);
});

test("the app is asked for its day lengths once for an edit event's commits, and by a before-edit call never", () => {
  const count = (unit: "days" | "hours", fresh: boolean, lists: Change[][], early = false): number => {
    reads.length = 0;
    changeForm(app, unit, "minute", early).lists(lists, fresh);
    return reads.length;
  };
  const span = task(0, { duration: 1 });
  expect(count("hours", true, [[span], [span], [task(0, { name: "x" })]])).toBe(1);
  expect(count("hours", true, [[task(0, { name: "x" })]])).toBe(0);
  expect(count("days", true, [[span]], true)).toBe(0);
  expect(count("hours", false, [[span]])).toBe(0);
  expect(count("hours", false, [[span]], true)).toBe(1);
});

test("a before-edit call measures with the schedule that is open: the day lengths are read again at an open", () => {
  lengths = { tasks: [480, 480], links: [] };
  const form = changeForm(app, "hours", "minute", true);
  const asked = (): unknown => form.lists([[task(1, { duration: 2 })]], false)[0]![0]!.set;
  expect(asked()).toEqual({ duration: 16 });
  lengths = { tasks: [450, 450], links: [] };
  expect(asked(), "the old schedule's days").toEqual({ duration: 16 });
  form.reread();
  expect(asked()).toEqual({ duration: 15 });
});
