// "Start on" and "finish on" as constraint kinds, and a date alone on a limit read on the task's calendar (decision 0017, rule 6; decision 0022, rule 9). The
// networks run on the built engine in Node (`openHeadless`: needs `src/wasm/`, scripts/build-web.sh) over a Monday-to-Friday calendar of 08:00 to 16:00; the
// first Monday is 2026-01-05.
import { test } from "node:test";
import { expect } from "@playwright/test";
import { foldConstraint, toSeparate, toTyped } from "../src/constraint-form.ts";
import { readFields } from "../src/field-map.ts";
import { openHeadless } from "../src/headless.ts";
import type { PlainTask } from "../src/adapters/plain.ts";
import type { Change } from "../src/events.js";

type Row = Record<string, unknown>;
const calendars = [{ key: "c", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }];
const task = (id: number, duration: number, more: Row = {}): PlainTask => ({ id, key: String(id), name: `T${id}`, start: "2026-01-05", duration, calendar: "c", ...more });
const results = (tasks: PlainTask[], links: { from: number; to: number }[] = []) => {
  const chart = openHeadless({ tasks, links, calendars });
  const all = chart.results();
  chart.free();
  return (id: number) => {
    const { start, finish, lateStart, lateFinish, float } = all.find((row) => row.key === String(id))!;
    return { start, finish, lateStart, lateFinish, float };
  };
};
const day = (date: string, from = "08:00", to = "16:00") => ({ start: `${date}T${from}`, finish: `${date}T${to}` });

test("a start on and a finish on that logic pushes past keep the bar and go negative by the days between", () => {
  const at = results(
    [task(1, 5), task(2, 1, { constraint: { type: "so", date: "2026-01-07" } }), task(3, 5), task(4, 1, { constraint: { type: "fo", date: "2026-01-07" } })],
    [{ from: 1, to: 2 }, { from: 3, to: 4 }],
  );
  const pushed = { ...day("2026-01-12"), lateStart: "2026-01-07T08:00", lateFinish: "2026-01-07T16:00", float: -3 };
  expect(at(2)).toEqual(pushed);
  expect(at(4)).toEqual(pushed);
});

test("a start on and a finish on that logic leaves before them hold the bar at the date, with no float", () => {
  const at = results(
    [task(1, 1), task(2, 1, { constraint: { type: "so", date: "2026-01-14" } }), task(3, 1), task(4, 1, { constraint: { type: "fo", date: "2026-01-14" } })],
    [{ from: 1, to: 2 }, { from: 3, to: 4 }],
  );
  const held = { ...day("2026-01-14"), lateStart: "2026-01-14T08:00", lateFinish: "2026-01-14T16:00", float: 0 };
  expect(at(2)).toEqual(held);
  expect(at(4)).toEqual(held);
});

test("a date alone on a finish limit is the last working minute of that day, in the typed form and in the member", () => {
  const at = results([task(1, 1, { constraint: { type: "fnet", date: "2026-01-07" } }), task(2, 1, { fnet: "2026-01-07" })]);
  for (const id of [1, 2]) expect(at(id), `fnet of task ${id}`).toMatchObject(day("2026-01-07"));
});

test("a date alone on a start limit is the first working minute of that day, in the typed form and in the member", () => {
  const long = task(9, 5, { start: "2026-01-12" }); // the project runs on to 2026-01-16, so the other two have room
  const at = results([task(3, 1, { constraint: { type: "snlt", date: "2026-01-07" } }), task(4, 1, { snlt: "2026-01-07" }), long]);
  for (const id of [3, 4]) expect(at(id), `snlt of task ${id}`).toMatchObject({ start: "2026-01-05T08:00", lateStart: "2026-01-07T08:00", float: 2 });
});

test("a typed so or fo comes back from a change as the same one constraint, and an update writes it as its two members", () => {
  const change = (set: Row, was: Row): Change => ({ record: "task", op: "update", key: "k", task: 0, set, was });
  expect(foldConstraint(change({ snet: "2026-01-07T08:00", snlt: "2026-01-07T08:00" }, { snet: null, snlt: null }))).toEqual(change({ constraint: { type: "so", date: "2026-01-07T08:00" } }, { constraint: null }));
  expect(foldConstraint(change({ fnet: "2026-01-07T16:00", fnlt: "2026-01-07T16:00" }, { fnet: null, fnlt: null }))).toEqual(change({ constraint: { type: "fo", date: "2026-01-07T16:00" } }, { constraint: null }));
  expect(toSeparate({ key: "k", constraint: { type: "so", date: "2026-01-07" } }, true)).toEqual({ key: "k", snet: "2026-01-07", snlt: "2026-01-07", fnet: null, fnlt: null });
  expect(toSeparate({ key: "k", constraint: { type: "fo", date: "2026-01-07" } }, true)).toEqual({ key: "k", snet: null, snlt: null, fnet: "2026-01-07", fnlt: "2026-01-07" });
  expect(toSeparate({ key: "k", constraint: { type: "so", date: "2026-01-07" } })).toEqual({ key: "k", snet: "2026-01-07", snlt: "2026-01-07" });
});

test("two bounds on one date are one constraint and two on different dates are not; a pin comes first and the bounds left over are counted", () => {
  expect(toTyped({ snet: "2026-01-07T08:00", snlt: "2026-01-08T08:00" })).toEqual({ constraint: { type: "snlt", date: "2026-01-08T08:00" }, leftOut: 1 });
  expect(toTyped({ snet: "2026-01-07T08:00", snlt: "2026-01-07T08:00", fnlt: "2026-01-09T16:00" })).toEqual({ constraint: { type: "so", date: "2026-01-07T08:00" }, leftOut: 1 });
  expect(toTyped({ mso: "2026-01-07T08:00", snet: "2026-01-07T08:00", snlt: "2026-01-07T08:00" })).toEqual({ constraint: { type: "mso", date: "2026-01-07T08:00" }, leftOut: 2 });
});

// A typed host reads a floor and a ceiling on one date as `so` or `fo` only when its `capabilities.kinds` is left out or lists the kind (the engine's profile counts them the same way).
const OLD = ["snet", "snlt", "fnet", "fnlt", "mso", "mfo", "alap"];
const both = (members: Row): Change => ({ record: "task", op: "update", key: "k", task: 0, set: members, was: {} });
const START = { snet: "2026-01-07T08:00", snlt: "2026-01-07T08:00" };
const FINISH = { fnet: "2026-01-07T16:00", fnlt: "2026-01-07T16:00" };
const typed = (members: Row, kinds?: readonly string[]) => readFields(undefined, () => undefined).changes([both(members)], kinds)[0]!.set;

test("a typed host whose kinds omit so and fo gets the single constraint it got before for both halves set in one commit", () => {
  expect(typed(START, OLD)).toEqual({ constraint: { type: "snlt", date: "2026-01-07T08:00" } });
  expect(typed(FINISH, OLD)).toEqual({ constraint: { type: "fnlt", date: "2026-01-07T16:00" } });
  expect(typed({ ...START, ...FINISH }, [...OLD, "fo"]), "so left out, fo listed: the reading order takes fo").toEqual({ constraint: { type: "fo", date: "2026-01-07T16:00" } });
});

test("a typed host whose kinds list so and fo, or leave kinds out, gets so and fo", () => {
  for (const kinds of [undefined, [...OLD, "so", "fo"], ["so", "fo"]]) {
    expect(typed(START, kinds), JSON.stringify(kinds)).toEqual({ constraint: { type: "so", date: "2026-01-07T08:00" } });
    expect(typed(FINISH, kinds), JSON.stringify(kinds)).toEqual({ constraint: { type: "fo", date: "2026-01-07T16:00" } });
  }
});
