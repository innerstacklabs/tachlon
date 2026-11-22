// The five read calls over a file, plain tasks or a document in Node, with no page (gantt#1250): `openHeadless` answers `whatIf`, `whatIfLonger`, `explain`, `health` and
// `criticalPath` from the built engine, and the answers are the ones the chart's commands give. Needs `packages/gantt/src/wasm/` (scripts/build-web.sh).
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { callTool, openHeadless, toolDefinitions } from "../src/headless.ts";
import { fromTasks } from "../src/adapters/plain.ts";
import { PlainDataError } from "../src/adapters/validate.ts";

// The office fit-out: 58 tasks, the survey (task 1) first, and "Layout approved" (task 3) a milestone that task 2 drives. Its finish is Thursday 2027-06-24.
const FITOUT = readFileSync(new URL("../tests/fixtures/office-fitout.xml", import.meta.url));

test("it runs with no window and no document", () => {
  expect(typeof window).toBe("undefined");
  expect(typeof document).toBe("undefined");
  expect(openHeadless(FITOUT).criticalPath().ok).toBe(true);
});

test("whatIfLonger moves the project finish by the known days, and whatIf slips a week", () => {
  const chart = openHeadless(FITOUT);
  // Three more days on the critical survey push the finish from 2027-06-24 to Tuesday 2027-06-29 (a weekend between).
  const longer = chart.whatIfLonger(1, 3);
  expect(longer).toMatchObject({ ok: true, finishWas: "2027-06-24 17:00", finishNow: "2027-06-29 17:00" });
  const slip = chart.whatIf([1]);
  expect(slip.ok && slip.finishWas).toBe("2027-06-24 17:00");
  expect(slip.ok && slip.finishNow).not.toBe("2027-06-24 17:00");
  expect(chart.whatIf([9999])).toMatchObject({ ok: false, reason: "unknownTask" });
});

test("explain names the chain that sets a start, nearest first", () => {
  const answer = openHeadless(FITOUT).explain(3);
  expect(answer).toMatchObject({ ok: true, start: "2027-01-21 17:00", capped: false });
  expect(answer.ok && answer.chain.map((link) => link.task)).toEqual([2, 1]);
});

test("health lists the tasks behind each count, and criticalPath the critical ones", () => {
  const chart = openHeadless(FITOUT);
  const checks = chart.health();
  expect(checks.ok && checks.counts.openStarts).toMatchObject({ count: 1, tasks: [{ task: 1 }] });
  expect(checks.ok && checks.counts.openEnds).toMatchObject({ count: 1, tasks: [{ task: 58 }] });
  const path = chart.criticalPath();
  expect(path.ok && path.critical.count).toBe(26);
  expect(path.ok && path.critical.tasks.slice(0, 3).map((t) => t.task)).toEqual([1, 2, 3]);
});

test("the answers change nothing: the same ask answers the same twice", () => {
  const chart = openHeadless(FITOUT);
  expect(chart.whatIfLonger(1, 3)).toEqual(chart.whatIfLonger(1, 3));
  expect(chart.criticalPath()).toEqual(chart.criticalPath());
});

test("the assistant's tools run over a file, propose is refused, and free releases the schedule", () => {
  const chart = openHeadless(FITOUT);
  expect(toolDefinitions("mcp").length).toBe(6);
  expect(callTool(chart, "whatIfLonger", { task: 1, days: 3 })).toEqual(chart.whatIfLonger(1, 3));
  expect(callTool(chart, "propose", { kind: "move", task: 3, start: "2027-02-01" })).toMatchObject({ ok: false, reason: "unknownTool" });
  chart.free();
  expect(() => chart.health()).toThrow();
});

test("a tool call after free() is a result, not a throw", () => {
  const chart = openHeadless(FITOUT);
  chart.free();
  expect(callTool(chart, "health", {})).toMatchObject({ ok: false, reason: "destroyed" });
});

test("a file that is not a schedule throws", () => {
  expect(() => openHeadless(new TextEncoder().encode("this is not a schedule\n"))).toThrow();
});

// Three plain tasks: "Long" (3 days) and "Short" (2) both start Monday 2027-01-04 and both drive "After" (1 day), so Long and After are critical and Short has a day to spare.
const PLAIN = {
  tasks: [
    { id: "short", name: "Short", start: "2027-01-04", duration: 2 },
    { id: "long", name: "Long", start: "2027-01-04", duration: 3 },
    { id: "after", name: "After", start: "2027-01-04", duration: 1 },
  ],
  links: [{ from: "short", to: "after" }, { from: "long", to: "after" }],
};
const CRITICAL = [{ task: 1, id: "long", name: "Long" }, { task: 2, id: "after", name: "After" }];

test("plain tasks and links open with no file, and criticalPath names the tasks worked by hand", () => {
  const chart = openHeadless(PLAIN);
  expect(chart.criticalPath()).toMatchObject({ ok: true, critical: { count: 2, tasks: CRITICAL } });
  expect(chart.leftOut).toEqual([]);
  expect(openHeadless({ tasks: [{ ...PLAIN.tasks[0]!, note: "x" }] }).leftOut.join("\n")).toContain("note");
  expect(openHeadless(FITOUT).leftOut).toEqual([]);
});

test("a document opens as the plain data it was made from does", () => {
  expect(openHeadless({ document: fromTasks(PLAIN).document }).criticalPath()).toEqual(openHeadless(PLAIN).criticalPath());
});

test("plain data the reader refuses throws a PlainDataError that lists the problem", () => {
  const bad = { tasks: [{ id: "a", name: "A", start: "not a date" }, { id: "a", name: "B", start: "2027-01-04" }] };
  expect(() => openHeadless(bad)).toThrow(PlainDataError);
  expect(() => openHeadless(bad)).toThrow(/2 problems[\s\S]*tasks\[0\]\.start[\s\S]*tasks\[1\]\.id/);
});

test("data that is not an object, has no tasks array, or has a links that is not one is a PlainDataError, not a TypeError", () => {
  for (const bad of [null, "x", 3, {}, { tasks: null }, { tasks: [], links: {} }, { tasks: [], links: null }]) expect(() => openHeadless(bad as never)).toThrow(PlainDataError);
});

test("project belongs to a file: with plain data or a document it is refused", () => {
  expect(() => openHeadless(PLAIN, "all")).toThrow(/project selects a project of a file/);
  expect(() => openHeadless({ document: fromTasks(PLAIN).document }, "all")).toThrow(/project selects a project of a file/);
});

// A net with one critical chain and a side path: `a` (1 day), `b` (2) and `c` (1) in a row, and `s` (1) then `m` (1) feeding `c`. All start Monday 2027-01-04; `c` starts Thursday, `s` has a day of total float and none free, and `m` a day of each.
const NET = {
  tasks: [["a", 1], ["b", 2], ["c", 1], ["s", 1], ["m", 1]] as const,
  links: [["a", "b"], ["b", "c"], ["s", "m"], ["m", "c"]] as const,
};
const PLAIN_NET = {
  tasks: NET.tasks.map(([id, days], i) => ({ id, name: id.toUpperCase(), start: "2027-01-04", duration: days, ...(i < 4 ? { key: `row "${id}"` } : {}) })),
  links: NET.links.map(([from, to]) => ({ from, to })),
};

// The same net as a P6 file on the plain data's calendar (Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00), a day being 8 hours.
const WEEK = "(0||CalendarData()((0||DaysOfWeek()((0||1()())" + [2, 3, 4, 5, 6].map((day) => `(0||${day}()((0||0(s|08:00|f|12:00)())(0||1(s|13:00|f|17:00)())))`).join("") + "(0||7()())))(0||Exceptions()())))";
const rows = (table: string, fields: string[], ...records: (string | number)[][]) => [`%T\t${table}`, `%F\t${fields.join("\t")}`, ...records.map((r) => `%R\t${r.join("\t")}`)];
const slot = (code: string) => NET.tasks.findIndex(([id]) => id === code) + 1;
const XER = new TextEncoder().encode(
  [
    "ERMHDR\t19.12\t2026-11-17 08:00\tProject\tadmin\tadmin\tdb\tProject Management\tUSD",
    ...rows("PROJECT", ["proj_id", "proj_short_name", "plan_start_date"], [1000, "P1", "2027-01-04 08:00"]),
    ...rows("CALENDAR", ["clndr_id", "default_flag", "clndr_name", "clndr_type", "day_hr_cnt", "clndr_data"], [100, "Y", "Standard", "CA_Base", 8, WEEK]),
    ...rows("PROJWBS", ["wbs_id", "proj_id", "proj_node_flag", "wbs_short_name", "wbs_name", "parent_wbs_id"], [100000, 1000, "Y", "W", "Project", ""], [100001, 1000, "N", "W1", "Work", 100000]),
    ...rows("TASK", ["task_id", "proj_id", "wbs_id", "clndr_id", "task_type", "status_code", "task_code", "task_name", "target_drtn_hr_cnt", "remain_drtn_hr_cnt"],
      ...NET.tasks.map(([id, days]) => [slot(id), 1000, 100001, 100, "TT_Task", "TK_NotStart", id, id.toUpperCase(), days * 8, days * 8])),
    ...rows("TASKPRED", ["task_pred_id", "task_id", "pred_task_id", "proj_id", "pred_proj_id", "pred_type", "lag_hr_cnt"], ...NET.links.map(([from, to], i) => [i + 1, slot(to), slot(from), 1000, 1000, "PR_FS", 0])),
    "%E",
  ].join("\n") + "\n",
);

test("results lists every task's dates and float, the same for plain data as for the same schedule read from a file", () => {
  const byCode = (list: ReturnType<ReturnType<typeof openHeadless>["results"]>) =>
    list.map(({ task: _slot, key: _key, ...answer }) => answer).sort((x, y) => (x.code! < y.code! ? -1 : 1));
  const fromPlain = openHeadless(PLAIN_NET).results();
  expect(byCode(fromPlain)).toEqual(byCode(openHeadless(XER).results()));
  // `s` ends Monday and has Tuesday to spare before `m`, which has Wednesday to spare before `c` on Thursday.
  const dates = (code: string) => fromPlain.find((t) => t.code === code)!;
  expect(dates("s")).toEqual({
    task: 3, key: 'row "s"', code: "s", start: "2027-01-04T08:00", finish: "2027-01-04T17:00", lateStart: "2027-01-05T08:00", lateFinish: "2027-01-05T17:00",
    float: 1, freeFloat: 0, critical: false, finished: false,
  });
  expect(dates("m")).toMatchObject({ start: "2027-01-05T08:00", lateStart: "2027-01-06T08:00", float: 1, freeFloat: 1, critical: false });
});

test("results carries the key the program gave each task, null for one with none, and a freed schedule throws", () => {
  const chart = openHeadless(PLAIN_NET);
  expect(chart.results().map((t) => t.key)).toEqual(['row "a"', 'row "b"', 'row "c"', 'row "s"', null]);
  chart.free();
  expect(() => chart.results()).toThrow();
});

test("plain data with unstartedRemaining on schedules an unstarted task for its remaining days", () => {
  const tasks = [{ id: "a", name: "A", start: "2027-01-04", duration: 5, remaining: 3 }];
  expect(openHeadless({ tasks, options: { unstartedRemaining: "on" } }).results()[0]).toMatchObject({ finish: "2027-01-06T17:00", critical: true });
  expect(() => openHeadless({ tasks })).toThrow(PlainDataError); // a remaining with no actual start is refused while the option is off
});

// The P6 fixture and the same tasks as plain data (tl#2081). Fixture columns, split on tabs: TASK 1 task_id, 13 task_code, 14 task_name, 22 target_drtn_hr_cnt, 18 remain_drtn_hr_cnt, 27 act_start_date, 28 act_end_date; TASKPRED 2 task_id, 3 pred_task_id, 6 pred_type, 7 lag_hr_cnt.
test("plain data with the actuals of a P6 file starts each task with an actual where the file does", () => {
  const xer = readFileSync(new URL("../tests/fixtures/one-project.xer", import.meta.url));
  const rows = xer.toString().split("\n").filter((line) => line.startsWith("%R\t")).map((line) => line.split("\t"));
  const task = rows.filter((c) => c[12]?.startsWith("TK_"));
  const code = Object.fromEntries(task.map((c) => [c[1], c[13]]));
  const tasks = task.map((c) => ({ id: c[13]!, name: c[14]!, start: "2024-01-01", duration: Number(c[22]) / 8, ...(c[27] ? { actualStart: c[27].replace(" ", "T") } : {}), ...(c[28] ? { actualFinish: c[28].replace(" ", "T") } : {}), ...(c[27] && !c[28] ? { remaining: Number(c[18]) / 8 } : {}) }));
  const links = rows.filter((c) => c[6]?.startsWith("PR_")).map((c) => ({ from: code[c[3]!]!, to: code[c[2]!]!, type: c[6]!.slice(3) as "FS", lag: Number(c[7]) / 8 }));
  const starts = (chart: ReturnType<typeof openHeadless>) => Array.from({ length: 14 }, (_, i) => (chart.explain(i) as { start?: string }).start);
  const actual = ["2024-01-01 08:00", "2024-01-06 08:00", "2024-01-11 08:00", "2024-01-12 08:00"]; // the four act_start_date values
  for (const chart of [openHeadless(xer), openHeadless({ tasks, links, dataDate: "2024-01-01T08:00" })]) expect(actual.filter((at) => !starts(chart).includes(at))).toEqual([]);
});

// tl#2081: the calendars reach the solve through plain data. 2027-01-04 is a Monday; a, then b, on a calendar of ten-hour weekdays.
test("a holiday on the critical path moves the finish one working day, counted on the calendar the plain data gives", () => {
  const tasks = [{ id: "a", name: "A", start: "2027-01-04", duration: 3, calendar: "ten" }, { id: "b", name: "B", start: "2027-01-04", duration: 1, calendar: "ten" }];
  const finish = (exceptions: { date: string; working: boolean }[]) => openHeadless({ tasks, links: [{ from: "a", to: "b" }], calendars: [{ key: "ten", workDays: [1, 2, 3, 4, 5], hoursPerDay: 10, exceptions }] }).results().map((t) => t.finish);
  expect(finish([])).toEqual(["2027-01-06T18:00", "2027-01-07T18:00"]);
  expect(finish([{ date: "2027-01-05", working: false }])).toEqual(["2027-01-07T18:00", "2027-01-08T18:00"]);
});

test("a profile fits plain data to what the host stores, and leftOut lists each change", () => {
  const data = { durationUnit: "minutes" as const, tasks: [{ id: "a", name: "A", start: "2027-01-04", duration: 90, snlt: "2027-01-08", fnlt: "2027-01-09" }] };
  const fit = openHeadless(data, undefined, { perTask: 1, durationStep: "hours" });
  expect(fit.leftOut).toHaveLength(2);
  expect(fit.leftOut[0]).toBe("1 task had a second date limit, which this host does not store; the later one was dropped");
  expect(fit.results()[0]!.finish).not.toBe(openHeadless(data).results()[0]!.finish);
});

// tl#2081: `dateForm: "day"` answers each date as `YYYY-MM-DD`. "work" ends at 16:00 on a weekday calendar; "night" runs a day of 24 hours, so it ends at 00:00 of the next day, which is still its own last day.
const DAYS = {
  calendars: [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, { key: "round", week: Array.from({ length: 7 }, (): [number, number][] => [[0, 1440]]), exceptions: [] }],
  tasks: [{ id: "work", name: "Work", start: "2027-01-04", duration: 1, calendar: "std" }, { id: "night", name: "Night", start: "2027-01-04", duration: 1, calendar: "round" }],
};
// "spare" drives "tail" beside the three-day "long", so it has two days of float: its late dates are 2027-01-06 where its early ones are 2027-01-04.
const FLOAT = {
  calendars: [DAYS.calendars[0]!],
  tasks: [["spare", 1], ["long", 3], ["tail", 1]].map(([id, duration]) => ({ id: id as string, name: id as string, start: "2027-01-04", duration: duration as number, calendar: "std" })),
  links: [{ from: "spare", to: "tail" }, { from: "long", to: "tail" }],
};
const answer = (chart: ReturnType<typeof openHeadless>, code: string) => chart.results().find((t) => t.code === code)!;

test("dateForm day answers a finish at 16:00 as its own day and a finish at T00:00 as the day before, lateStart and lateFinish too", () => {
  const chart = openHeadless({ ...DAYS, dateForm: "day" });
  expect(answer(openHeadless(DAYS), "night")).toMatchObject({ start: "2027-01-04T00:00", finish: "2027-01-05T00:00" });
  expect(answer(openHeadless(DAYS), "work")).toMatchObject({ finish: "2027-01-04T16:00", lateFinish: "2027-01-04T16:00" });
  expect(answer(chart, "work")).toMatchObject({ start: "2027-01-04", finish: "2027-01-04", lateStart: "2027-01-04", lateFinish: "2027-01-04" });
  expect(answer(chart, "night")).toMatchObject({ start: "2027-01-04", finish: "2027-01-04", lateStart: "2027-01-04", lateFinish: "2027-01-04" });
});

test("dateForm day answers each date from its own member: a task with float has late days that differ from its early days", () => {
  expect(answer(openHeadless(FLOAT), "spare")).toMatchObject({ start: "2027-01-04T08:00", finish: "2027-01-04T16:00", lateStart: "2027-01-06T08:00", lateFinish: "2027-01-06T16:00" });
  expect(answer(openHeadless({ ...FLOAT, dateForm: "day" }), "spare")).toMatchObject({ start: "2027-01-04", finish: "2027-01-04", lateStart: "2027-01-06", lateFinish: "2027-01-06" });
});

test("with no dateForm, or minute, the answers are the minutes form", () => {
  const minutes = JSON.stringify(openHeadless(DAYS).results());
  expect(minutes).toContain('"finish":"2027-01-05T00:00"');
  expect(JSON.stringify(openHeadless({ ...DAYS, dateForm: "minute" }).results())).toBe(minutes);
});
