// A change in the unit and date form the host mounted with (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7). The app stamps each change with the working day that measures it; what an
// `edit` event and a before-edit call carry in a page is `tests/before-edit.spec.ts`.
import { formChanges } from "../src/change-form.ts";
import type { Stamped } from "../src/change-set.js";
import type { Change } from "../src/events.js";
import { test } from "node:test";
import { expect } from "@playwright/test";

const task = (day: number, set: Record<string, unknown>, was: Record<string, unknown> = set, op: Change["op"] = "update"): Stamped => ({ record: "task", op, key: null, task: 0, dayMinutes: day, set, was });
const link = (day: number, lag: number, op: Change["op"] = "update"): Stamped => ({ record: "link", op, key: null, link: 0, dayMinutes: day, ...(op === "delete" ? { was: { lag } } : { set: { lag }, was: { lag: 0 } }) });
const hours = (change: Stamped): unknown => formChanges([change], "hours", "minute").map((c) => c.set ?? c.was)[0];
const bare = ({ dayMinutes: _day, ...change }: Stamped): Change => change;

test("durations pass through in days, and the stamp is removed whatever the unit", () => {
  const change = task(480, { duration: 2.5, remaining: 1 });
  expect(formChanges([change], "days", "minute")).toEqual([bare(change)]);
  expect(formChanges([change], undefined, undefined)).toEqual([bare(change)]);
  expect(formChanges([link(450, 1)], "hours", "day")[0]).not.toHaveProperty("dayMinutes");
});

test("hours and minutes on a 450-minute and a 480-minute calendar, in set and in was", () => {
  const [eight, seven] = [task(480, { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null }), task(450, { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null })];
  expect(formChanges([eight, seven], "hours", "minute")).toEqual([bare(task(480, { duration: 16, remaining: 4 }, { duration: 8, remaining: null })), bare(task(450, { duration: 15, remaining: 3.75 }, { duration: 7.5, remaining: null }))]);
  expect(formChanges([eight, seven], "minutes", "minute").map((c) => c.set)).toEqual([{ duration: 960, remaining: 240 }, { duration: 900, remaining: 225 }]);
});

test("a lag is measured by the day its change carries, whichever calendar the app counted it in", () => {
  expect([hours(link(450, 1)), hours(link(1440, 1)), hours(link(480, 2))]).toEqual([{ lag: 7.5 }, { lag: 24 }, { lag: 16 }]);
});

test("a task delete, the link delete that follows from it and the undo of both are measured by the day they carry", () => {
  expect(hours(task(450, { duration: 1 }, { duration: 1 }, "delete"))).toEqual({ duration: 7.5 });
  expect(hours({ ...link(450, 2, "delete"), cascade: true })).toEqual({ lag: 15 });
  expect(hours(task(450, { duration: 1 }, {}, "create"))).toEqual({ duration: 7.5 });
  expect(hours(link(450, 2, "create"))).toEqual({ lag: 15 });
});

test("a planned task insert carries no stamp and no duration, and passes through as it is", () => {
  const planned: Stamped = { record: "task", op: "create", key: null, task: 9, set: { name: "T" } };
  expect(formChanges([planned], "hours", "minute")).toEqual([planned]);
});

test("dateForm day: a start-kind date is its day, a finish-kind date the day of its instant less one minute", () => {
  const dates = task(480, { snet: "2026-01-12T17:00", snlt: "2026-01-13T00:00", fnet: "2026-01-12T17:00", fnlt: "2026-01-13T00:00", actualStart: null, actualFinish: "2026-03-01T00:00", name: "T" }, { mfo: "2026-01-01T00:00", mso: "2026-01-01T00:00" });
  expect(formChanges([dates], "days", "day")).toEqual([bare(task(480, { snet: "2026-01-12", snlt: "2026-01-13", fnet: "2026-01-12", fnlt: "2026-01-12", actualStart: null, actualFinish: "2026-02-28", name: "T" }, { mfo: "2025-12-31", mso: "2026-01-01" }))]);
  const schedule: Change = { record: "schedule", op: "update", key: null, set: { dataDate: "2026-01-12T00:00" }, was: { dataDate: "2026-01-09T08:00" } };
  expect(formChanges([schedule], "days", "day")).toEqual([{ ...schedule, set: { dataDate: "2026-01-12" }, was: { dataDate: "2026-01-09" } }]);
  const baseline: Change = { record: "schedule", op: "update", key: null, set: { baseline: { projectFinish: "2026-01-13T00:00", dataDate: "2026-01-12T08:00" } }, was: { baseline: null } };
  expect(formChanges([baseline], "days", "day")[0]!.set, "the baseline's project finish is a finish, its data date a start").toEqual({ baseline: { projectFinish: "2026-01-12", dataDate: "2026-01-12" } });
  expect(formChanges([baseline], "days", "minute"), 'dateForm "minute" leaves it as the engine wrote it').toEqual([baseline]);
  expect(formChanges([dates], "days", "minute"), 'dateForm "minute" leaves a date as the engine wrote it').toEqual([bare(dates)]);
});
