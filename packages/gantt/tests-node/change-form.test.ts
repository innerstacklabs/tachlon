// A change in the unit and date form the host mounted with (decision 0022, rules 2.3 and 2.4; decision 0017, rules 5 and 7). What an `edit` event carries in a page is `tests/events.spec.ts`.
import { changeForm, formChanges, readDays } from "../src/change-form.ts";
import type { PublicApp } from "../src/api.js";
import type { Change } from "../src/events.js";
import { test } from "node:test";
import { expect } from "@playwright/test";

const week = (minutes: number): [number, number][][] => [0, 1, 2, 3, 4].map((): [number, number][] => [[480, 480 + minutes]]).concat([[], []]);
const doc = { calendars: [{ week: week(480) }, { week: week(450), key: "short" }], lag_calendar: "successor", tasks: [{ key: "a", calendar: 0 }, { key: "b", calendar: 1 }], links: [{ key: "l", pred: 0, succ: 1, lag_calendar: null }] };
const reads: number[] = [];
const app = { save: () => (reads.push(1), JSON.stringify(doc)) } as unknown as PublicApp;
const update = (key: string, set: Record<string, unknown>, was: Record<string, unknown> = set): Change => ({ record: "task", op: "update", key, set, was });
const sets = (changes: Change[]): unknown[] => changes.map((c) => c.set);

test("durations pass through in days", () => {
  const change = update("b", { duration: 2.5, remaining: 1 });
  expect(formChanges([change], "days", "minute", () => 1)).toEqual([change]);
  expect(formChanges([change], undefined, undefined, () => 1)).toEqual([change]);
});

test("hours and minutes on a 450-minute and a 480-minute calendar, in set and in was", () => {
  const day = readDays(app);
  const [eight, seven] = [update("a", { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null }), update("b", { duration: 2, remaining: 0.5 }, { duration: 1, remaining: null })];
  expect(formChanges([eight, seven], "hours", "minute", day)).toEqual([update("a", { duration: 16, remaining: 4 }, { duration: 8, remaining: null }), update("b", { duration: 15, remaining: 3.75 }, { duration: 7.5, remaining: null })]);
  expect(sets(formChanges([eight, seven], "minutes", "minute", day))).toEqual([{ duration: 960, remaining: 240 }, { duration: 900, remaining: 225 }]);
});

test("a lag is in days of the link's lag calendar: the successor's, the predecessor's, 24 hours, or the one it names", () => {
  const day = readDays(app);
  const lag = (set: Record<string, unknown>, key: string | null = "l"): Change => ({ record: "link", op: "update", key, set, was: { lag: 0 } });
  const hours = (change: Change): unknown => (formChanges([change], "hours", "minute", day).map((c) => c.set ?? c.was)[0] as { lag: number }).lag;
  expect(hours(lag({ lag: 1 }))).toBe(7.5);
  expect(hours({ record: "link", op: "create", key: "n", set: { from: "a", to: "b", lag: 1, lagCalendar: "predecessor" } })).toBe(8);
  expect(hours({ record: "link", op: "delete", key: "n", was: { from: "a", to: "b", lag: 1, lagCalendar: "24h" } })).toBe(24);
  expect(hours({ record: "link", op: "delete", key: "n", was: { from: "b", to: "a", lag: 2, lagCalendar: { calendar: "short" } } })).toBe(15);
  expect(hours({ record: "link", op: "create", key: "n", set: { from: "b", to: "a", lag: 1 } })).toBe(8);
});

test("a created or deleted row is measured by the calendar it names, an unknown key by the first calendar", () => {
  const day = readDays(app);
  const gone: Change = { record: "task", op: "delete", key: "z", was: { duration: 1, calendar: "short" } };
  const made: Change = { record: "task", op: "create", key: "y", set: { duration: 1, calendar: { calendar: 1 } } };
  expect(formChanges([gone, made, update("z", { duration: 1 })], "minutes", "minute", day).map((c) => (c.set ?? c.was)!.duration)).toEqual([450, 450, 480]);
});

test("dateForm day: a start-kind date is its day, a finish-kind date the day of its instant less one minute", () => {
  const dates = update("a", { snet: "2026-01-12T17:00", snlt: "2026-01-13T00:00", fnet: "2026-01-12T17:00", fnlt: "2026-01-13T00:00", actualStart: null, actualFinish: "2026-03-01T00:00", name: "T" }, { mfo: "2026-01-01T00:00", mso: "2026-01-01T00:00" });
  expect(formChanges([dates], "days", "day", () => 0)).toEqual([update("a", { snet: "2026-01-12", snlt: "2026-01-13", fnet: "2026-01-12", fnlt: "2026-01-12", actualStart: null, actualFinish: "2026-02-28", name: "T" }, { mfo: "2025-12-31", mso: "2026-01-01" })]);
  const schedule: Change = { record: "schedule", op: "update", key: null, set: { dataDate: "2026-01-12T00:00" }, was: { dataDate: "2026-01-09T08:00" } };
  expect(formChanges([schedule], "days", "day", () => 0)).toEqual([{ ...schedule, set: { dataDate: "2026-01-12" }, was: { dataDate: "2026-01-09" } }]);
});

test('dateForm "minute" leaves a date as the engine wrote it', () => {
  const change = update("a", { snet: "2026-01-12T17:00", actualFinish: "2026-01-13T00:00" });
  expect(formChanges([change], "days", "minute", () => 0)).toEqual([change]);
});

test("the saved document is read for a duration or a lag in another unit, and only by the edit event", () => {
  const read = (unit: "days" | "hours", fresh: boolean, change: Change, early = false): number => {
    reads.length = 0;
    changeForm(app, unit, "minute", early)([change], fresh);
    return reads.length;
  };
  expect(read("hours", true, update("a", { duration: 1 }))).toBe(1);
  expect(read("hours", true, update("a", { name: "x" }))).toBe(0);
  expect(read("days", true, update("a", { duration: 1 }), true)).toBe(0);
  expect(read("hours", false, update("a", { duration: 1 }))).toBe(0);
  expect(read("hours", false, update("a", { duration: 1 }), true)).toBe(1);
});

