// The plain task adapter (tl#1079): `fromTasks` reads `{ tasks, links }` into a document v1. What the test expects is worked
// out here from the input's own text, not by calling the adapter: 2026-03-02 is a Monday, so it is the origin; a start is
// minutes from it (a day is 1440); a working day of duration or lag is 480 minutes; progress 50 is 50 percent.
import { fromTasks, type PlainLink, type PlainTask } from "../src/adapters/plain.ts";
import { fromDhtmlx } from "../src/adapters/dhtmlx.ts";
import { PlainDataError, validate } from "../src/adapters/validate.ts";
import { buildDataset } from "../../../demos/bench/dataset.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const tasks: PlainTask[] = [
  { id: "site", name: "Site works", start: "2026-03-02" },
  { id: "strip", name: "Strip out", start: "2026-03-02T08:00", duration: 3, parent: "site", progress: 50 },
  { id: "walls", name: "Partitions", start: "2026-03-05", end: "2026-03-09", parent: "site", progress: 25, colour: "red" },
  { id: 7, name: "Handover", start: "2026-03-12", duration: 0, progress: 100 },
];
const links: PlainLink[] = [
  { from: "strip", to: "walls", type: "SS", lag: 1 },
  { from: "walls", to: 7 },
  { from: "site", to: 7, type: "FF" },
];
const task = { calendar: 0, actual_start: null, actual_finish: null, remaining: null, wbs_code: null, activity_codes: [] };

test("plain_tasks_and_links_become_a_document", () => {
  const { document, leftOut } = fromTasks({ tasks, links }, { name: "fit-out" });
  expect(document).toMatchObject({ format: "gantt", version: 1, name: "fit-out", origin: Date.UTC(2026, 2, 2) / 86_400_000, notes: leftOut });
  expect(document.tasks).toEqual([
    { ...task, name: "Site works", code: "site", parent: null, outline_level: 1, duration: 0, snet: null, progress: 0 },
    { ...task, name: "Strip out", code: "strip", parent: 0, outline_level: 2, duration: 3 * 480, snet: 8 * 60, progress: 50 },
    { ...task, name: "Partitions", code: "walls", parent: 0, outline_level: 2, duration: 4 * 480, snet: 3 * 1440, progress: 25 },
    { ...task, name: "Handover", code: "7", parent: null, outline_level: 1, duration: 0, snet: 10 * 1440, progress: 100 },
  ]);
  expect(document.links).toEqual([
    { pred: 1, succ: 2, kind: "SS", lag: 480, lag_calendar: null },
    { pred: 2, succ: 3, kind: "FS", lag: 0, lag_calendar: null },
  ]);
  expect(leftOut).toEqual([
    "Plain JSON summary tasks take their dates and progress from their children: the own start, end, duration, progress and constraint of 1 summary task are not read.",
    "Plain JSON 1 link touches a summary task, which a schedule here cannot link; it is left out.",
    'Plain JSON task field "colour" is not read (1).',
  ]);
});

test("the_dhtmlx_lines_keep_their_word_and_a_bad_plain_input_is_an_error", () => {
  expect(fromDhtmlx({ data: [{ id: 1, color: "red" }] }).leftOut).toContain('DHTMLX task field "color" is not read (1).');
  expect(() => fromTasks({ tasks: [{ id: 1, name: "A", start: "2026-03-02" }], links: [{ from: 1, to: 1, type: "XX" as "FS" }] })).toThrow(/links\[0\].type: expected one of/);
  expect(() => fromTasks({ tasks: {} } as never)).toThrow(/tasks: expected an array of tasks/);
});

test("a_task_color_of_the_form_rrggbb_reaches_the_document_and_any_other_value_is_left_out_by_key", () => {
  const read = (color: unknown) => fromTasks({ tasks: [{ id: 1, name: "A", start: "2026-03-02", duration: 1, color: color as string }] });
  expect(read("#2e7d32").document.tasks[0]).toMatchObject({ color: "#2e7d32" });
  expect(read("#2E7D32").leftOut).toEqual([]);
  for (const bad of ["red", "#2e7d3280", "#fff", 5]) {
    expect(read(bad).document.tasks[0]).not.toHaveProperty("color");
    expect(read(bad).leftOut).toEqual(['Plain JSON task field "color" is not read (1).']);
  }
});

test("a_task_label_reaches_the_document_and_a_label_that_is_not_text_is_left_out_by_key", () => {
  const read = (label: unknown) => fromTasks({ tasks: [{ id: 1, name: "A", start: "2026-03-02", duration: 1, label: label as string }] });
  expect(read("Phase A").document.tasks[0]).toMatchObject({ name: "A", label: "Phase A" });
  expect(read("Phase A").leftOut).toEqual([]);
  expect(read(5).document.tasks[0]).not.toHaveProperty("label");
  expect(read(5).leftOut).toEqual(['Plain JSON task field "label" is not read (1).']);
});

test("a_task_key_reaches_the_document_and_a_key_that_is_not_text_is_refused", () => {
  const input = (key: unknown) => ({ tasks: [{ id: 1, name: "A", start: "2026-03-02", duration: 1, key: key as string }, { id: 2, name: "B", start: "2026-03-02", duration: 1 }] });
  const uuid = "6f1c2d5e-0b7a-4c1e-9a53-2d8f7e4b1a90";
  const read = fromTasks(input(uuid));
  expect(read.document.tasks[0]).toMatchObject({ key: uuid });
  expect(read.document.tasks[1]).not.toHaveProperty("key");
  expect(read.leftOut).toEqual([]);
  expect(validate(input(42)).ok).toBe(false);
  expect(() => fromTasks(input(42))).toThrow(/tasks\[0\].key: expected text/);
});

test("a_link_key_reaches_the_document_a_repeat_or_empty_key_is_named_and_a_repeated_pair_drops_its_key", () => {
  const task = (id: number, key?: string) => ({ id, name: `T${id}`, start: "2026-03-02", duration: 1, ...(key === undefined ? {} : { key }) });
  const input = (links: PlainLink[], tasks = [task(1), task(2), task(3)]) => ({ tasks, links });
  const [a, b] = ["e2a9c7f1-5d38-4b06-9c4e-8f1a6d3b2e75", "3a7e90d1-c4b2-4e68-b1f5-6d02a8c9e347"];
  const read = fromTasks(input([{ from: 1, to: 2, key: a }, { from: 2, to: 3 }, { from: 1, to: 2, key: b }]));
  expect(read.document.links[0]).toMatchObject({ key: a });
  expect(read.document.links[1]).not.toHaveProperty("key");
  expect(read.document.links).toHaveLength(2);
  expect(read.leftOut).toEqual(["Plain JSON 1 link repeats a pair of tasks already linked; the first is kept."]);
  expect(validate(input([{ from: 1, to: 2, key: 7 as unknown as string }])).ok).toBe(false);
  const repeated = validate(input([{ from: 1, to: 2, key: a }, { from: 2, to: 3, key: a }], [task(1, a), task(2, a), task(3)]));
  expect(repeated.ok ? [] : repeated.issues.map((i) => `${i.path}: ${i.got}`)).toEqual([`tasks[1].key: string "${a}", also tasks[0].key`, `links[1].key: string "${a}", also links[0].key`]);
  expect(validate(input([{ from: 1, to: 2, key: "" }])).ok).toBe(false);
});

// The old path, kept here only: write the DHTMLX shape and read it with `fromDhtmlx`. `fromTasks` must give the same document and `leftOut`, or the same error.
type Plain = { tasks: PlainTask[]; links?: PlainLink[] };
const KINDS = { FS: "0", SS: "1", FF: "2", SF: "3" } as const;
function viaDhtmlx({ tasks, links = [] }: Plain) {
  const data = tasks.map(({ id, name, start, end, duration, parent, progress, ...rest }) => ({ ...rest, id, text: name, start_date: start, end_date: end, duration, parent, progress: typeof progress === "number" ? progress / 100 : progress }));
  const ls = links.map(({ from, to, type, lag, ...rest }) => ({ ...rest, source: from, target: to, type: KINDS[type ?? "FS"], lag }));
  return fromDhtmlx({ data, links: ls }, { label: "Plain JSON" });
}
const same = (input: Plain) => {
  const run = (read: (d: Plain) => unknown) => { try { return JSON.stringify(read(input)); } catch (e) { return `throws ${(e as Error).message}`; } };
  expect(run((d) => fromTasks(d))).toBe(run(viaDhtmlx));
};
const bench = (shape: "tree4" | "chain"): Plain => {
  const d = buildDataset(shape, 10_000, 1n);
  return { tasks: d.data.map((t) => ({ id: t.id, name: t.text, start: t.start ?? d.start, duration: t.duration, parent: t.parent })) as PlainTask[], links: d.links.map((l) => ({ from: l.source, to: l.target })) };
};
const t0 = { id: 1, name: "A", start: "2026-03-02" };

test("fromTasks_equals_the_dhtmlx_path_on_the_bench_datasets_and_the_fixtures", () => {
  for (const input of [bench("tree4"), bench("chain"), { tasks, links }]) {
    expect(() => fromTasks(input)).not.toThrow();
    same(input);
  }
  same({ tasks: [{ ...t0, name: "", type: "milestone", $x: 1, text: "t", extra: 1 }, { id: 2, name: "B", start: "2026-03-03T09:30", end: "2026-03-05", parent: 1, progress: 100, duration: 2 }, { id: 3, name: "C", start: "2026-03-03" }, { id: 4, name: "D", start: "2026-03-03", end: "2026-03-04", parent: 3 }], links: [{ from: 2, to: 4, id: "k", z: 1 }, { from: 2, to: 4 }, { from: 3, to: 2, type: "SF", lag: -1.5 }, { from: 4, to: 2 }] });
  same({ tasks: [{ ...t0, id: "", parent: "" }] });
});

test("validate_refuses_what_the_dhtmlx_path_throws_for_and_fromTasks_throws_a_PlainDataError", () => {
  const bad: Plain[] = [...[{ ...t0, id: 0 }, { ...t0, start: "2026-02-31" }, { ...t0, start: "2026-03-05", end: "2026-03-02" }].map((task) => ({ tasks: [task] })), { tasks: [{ ...t0, parent: 2 }, { id: 2, name: "B", start: "2026-03-02", parent: 1 }] }, { tasks: [t0], links: [{ from: 1, to: 1, id: "L" }] }];
  for (const input of bad) {
    expect(validate(input).ok).toBe(false);
    expect(() => fromTasks(input)).toThrow(PlainDataError);
  }
});

// Minutes from the origin (Monday 2026-03-02): 2026-03-06 is day 4, and 09:00 is 540 minutes into it.
const FRI = 4 * 1440;
const limitOf = (task: Record<string, unknown>) => fromTasks({ tasks: [t0, { id: 2, name: "B", start: "2026-03-02", duration: 1, ...task }] }).document.tasks[1]!;

test("each_limit_key_of_a_plain_task_lands_in_the_documents_limit", () => {
  const at = "2026-03-06T09:00";
  expect(limitOf({ snet: "2026-03-04" })).toMatchObject({ snet: 2 * 1440 });
  expect(limitOf({ snet: "2026-03-04" })).not.toHaveProperty("limit");
  expect(limitOf({ snlt: at }).limit).toEqual({ snlt: FRI + 540 });
  expect(limitOf({ fnet: at }).limit).toEqual({ fnet: FRI + 540 });
  expect(limitOf({ fnlt: at }).limit).toEqual({ fnlt: FRI + 540 });
  expect(limitOf({ mso: at }).limit).toEqual({ pin_start: FRI + 540 });
  expect(limitOf({ mfo: at }).limit).toEqual({ pin_finish: FRI + 540 });
  expect(limitOf({ alap: true }).limit).toEqual({ alap: true });
  expect(limitOf({ alap: false })).not.toHaveProperty("limit");
  expect(limitOf({ snlt: at, fnlt: at, alap: true }).limit).toEqual({ snlt: FRI + 540, fnlt: FRI + 540, alap: true });
});

test("a_date_alone_on_a_finish_limit_is_the_end_of_that_day_and_on_a_start_limit_its_start", () => {
  const day = "2026-03-06";
  for (const [key, field] of [["fnet", "fnet"], ["fnlt", "fnlt"], ["mfo", "pin_finish"]] as const) expect(limitOf({ [key]: day }).limit).toEqual({ [field]: FRI + 1440 });
  expect(limitOf({ snlt: day }).limit).toEqual({ snlt: FRI });
  expect(limitOf({ mso: day }).limit).toEqual({ pin_start: FRI });
});

test("the_plain_path_and_the_dhtmlx_path_give_the_same_limit_for_the_same_input", () => {
  for (const key of ["snet", "snlt", "fnet", "fnlt", "mso", "mfo", "alap"]) {
    const plain = fromTasks({ tasks: [{ ...t0, duration: 1, [key]: key === "alap" ? true : "2026-03-06T09:00" }] });
    const old = fromDhtmlx({ data: [{ id: 1, text: "A", start_date: "2026-03-02", duration: 1, constraint_type: key, constraint_date: key === "alap" ? undefined : "2026-03-06 09:00" }] });
    expect(plain.document.tasks, key).toEqual(old.document.tasks);
    expect(plain.document.origin, key).toBe(old.document.origin);
  }
});

const second = (task: Record<string, unknown>) => fromTasks({ tasks: [t0, { id: 2, name: "B", start: "2026-03-02", duration: 2, ...task }] }).document.tasks[1]!; // tl#2081: minutes from the origin, so Tuesday 2026-03-03 is day 1 and Saturday 03-07 day 5; a working day runs 08:00 to 17:00

test("an_actual_start_and_finish_reach_the_document_and_a_date_alone_is_read_by_the_working_day", () => {
  expect(second({ actualStart: "2026-03-03T09:30", actualFinish: "2026-03-04T11:00" })).toMatchObject({ actual_start: 1440 + 570, actual_finish: 2 * 1440 + 660 });
  expect(second({ actualStart: "2026-03-03", actualFinish: "2026-03-04" })).toMatchObject({ actual_start: 1440 + 480, actual_finish: 2 * 1440 + 1020 });
  expect(second({ actualStart: "2026-03-07", actualFinish: "2026-03-07" })).toMatchObject({ actual_start: 5 * 1440, actual_finish: 6 * 1440 });
  expect(second({})).toMatchObject({ actual_start: null, actual_finish: null, remaining: null });
  expect(second({ actualStart: "2026-03-03", remaining: 1.5 })).toMatchObject({ remaining: 720 }); // working days, as duration is
});

test("resume_lands_in_the_documents_limit_beside_the_other_limits", () => {
  expect(second({ actualStart: "2026-03-03", resume: "2026-03-06" }).limit).toEqual({ resume: FRI });
  expect(second({ actualStart: "2026-03-03", resume: "2026-03-06T09:00", snlt: "2026-03-10" }).limit).toEqual({ snlt: 8 * 1440, resume: FRI + 540 });
});

test("the_data_date_reaches_the_document_and_moves_the_origin_when_it_is_the_earliest_date", () => {
  const read = (dataDate?: string) => fromTasks({ tasks: [t0], dataDate }).document;
  expect(read("2026-03-04T08:00").data_date).toBe(2 * 1440 + 480);
  expect(read("2026-03-04").data_date).toBe(2 * 1440);
  expect(read("2026-02-24")).toMatchObject({ origin: Date.UTC(2026, 1, 23) / 86_400_000, data_date: 1440 });
});

test("the_options_reach_the_document_as_one_object_and_are_absent_when_not_given", () => {
  const options = { startedLateStart: "project", unstartedRemaining: "on" } as const;
  expect(fromTasks({ tasks: [t0], options }).document.options).toEqual(options);
  expect(fromTasks({ tasks: [t0], options: { unstartedRemaining: "on" } }).document.options).toEqual({ unstartedRemaining: "on" });
  expect(fromTasks({ tasks: [t0] }).document).not.toHaveProperty("options");
});

// tl#2081: the calendars and the unit. The origin is Monday 2026-03-02, so Wednesday 03-04 is day 2 and Saturday 03-07 day 5.
const std = { key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 };
const ten = { key: "ten", workDays: [1, 2, 3, 4], hoursPerDay: 10 };
const withData = (data: object, task: object = {}) => fromTasks({ tasks: [{ ...t0, ...task }], ...data } as never);

test("a_task_on_a_calendar_of_four_ten_hour_days_counts_a_day_as_600_minutes_and_the_document_keeps_the_week_and_the_key", () => {
  const { document } = fromTasks({ tasks: [{ ...t0, duration: 2, calendar: "ten" }, { id: 2, name: "B", start: "2026-03-02", duration: 1 }], calendars: [std, ten] } as never);
  expect(document.tasks.map((task) => [task.calendar, task.duration])).toEqual([[1, 1200], [0, 480]]);
  expect(document.calendars[1]).toEqual({ week: [0, 1, 2, 3, 4, 5, 6].map((day) => (day < 4 ? [[480, 1080]] : [])), exceptions: [], key: "ten" });
  expect(document.calendars[0]).toMatchObject({ week: Array.from({ length: 7 }, (_, day) => (day < 5 ? [[480, 960]] : [])), key: "std" });
  const plain = fromTasks({ tasks: [t0] }).document; // no new member: the one Monday to Friday calendar with no key, and a lag on the successor's
  expect(plain.calendars).toEqual([{ week: [0, 1, 2, 3, 4, 5, 6].map((day) => (day < 5 ? [[480, 720], [780, 1020]] : [])), exceptions: [] }]);
  expect(plain.lag_calendar).toBe("successor");
  expect(fromTasks({ tasks: [t0], calendars: [] }).document.calendars).toEqual(plain.calendars); // an empty list is no calendars, not a crash
});

test("the_unit_sets_what_a_duration_counts_a_fraction_of_a_minute_rounds_and_a_week_calendars_day_is_its_average", () => {
  const length = (duration: number, data: object = {}) => withData(data, { duration }).document.tasks[0]!.duration;
  expect(length(12, { durationUnit: "hours" })).toBe(720);
  expect(length(90, { durationUnit: "minutes" })).toBe(90);
  expect(length(89.6, { durationUnit: "minutes" })).toBe(90);
  expect(length(1, { durationUnit: "days" })).toBe(480);
  const six = { key: "six", week: [...Array.from({ length: 5 }, () => [[480, 720], [780, 1020]]), [[480, 720]], []] }; // five days of 480 and a Saturday of 240: 2,640 over six days
  expect(length(1, { calendars: [six] })).toBe(440);
  const started = (task: object) => withData({ calendars: [ten], durationUnit: "hours" }, { duration: 5, calendar: "ten", actualStart: "2026-03-02", ...task }).document.tasks[0]!;
  expect(started({ remaining: 2 })).toMatchObject({ remaining: 120 });
  expect(started({ actualFinish: "2026-03-02" })).toMatchObject({ actual_start: 480, actual_finish: 1080 }); // the end of the ten-hour day, not the standard day's 17:00
});

test("a_holiday_and_an_extra_working_day_land_at_their_days_from_the_origin_and_one_before_the_origin_is_left_out", () => {
  const exceptions = [{ date: "2026-03-04", working: false }, { date: "2026-03-07", working: true }, { date: "2026-02-27", working: false }];
  const { document, leftOut } = withData({ calendars: [{ ...ten, exceptions, colour: "red" }] }, { duration: 1 });
  expect(document.calendars[0]!.exceptions).toEqual([[2, []], [5, [[480, 1080]]]]);
  expect(leftOut).toEqual(["Plain JSON 1 calendar exception falls before the schedule's first Monday, where no work starts; it is left out.", 'Plain JSON calendar field "colour" is not read (1).']);
});

test("a_lag_day_is_counted_on_the_calendar_the_lag_calendar_names_and_a_key_becomes_the_calendars_position", () => {
  const read = (lagCalendar?: unknown, durationUnit?: string) => fromTasks({ tasks: [{ ...t0, calendar: "std" }, { id: 2, name: "B", start: "2026-03-02", duration: 1, calendar: "ten" }], links: [{ from: 1, to: 2, lag: 1 }], calendars: [std, ten], lagCalendar, durationUnit } as never).document;
  expect(read().links[0]!.lag).toBe(600); // the successor's day
  expect(read("predecessor").links[0]!.lag).toBe(480);
  expect(read("24h").links[0]!.lag).toBe(1440);
  expect(read({ calendar: "ten" }).links[0]!.lag).toBe(600);
  expect(read(undefined, "hours").links[0]!.lag).toBe(60);
  expect([read("predecessor").lag_calendar, read({ calendar: "ten" }).lag_calendar]).toEqual(["predecessor", { calendar: 1 }]);
});
