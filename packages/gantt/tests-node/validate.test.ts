// `validate` and `fromTasks` name every mistake in plain task data (tl#1249). The schema file is evaluated here by a small
// subset evaluator (type, required, properties, items, enum, oneOf, pattern, minimum, exclusiveMinimum, maximum, $ref; integer as a whole number), so the schema and `validate`
// are held to the same verdict on the same shapes.
import { fromTasks } from "../src/adapters/plain.ts";
import { PlainDataError, validate } from "../src/adapters/validate.ts";
import schema from "../src/adapters/plain.schema.json" with { type: "json" };
import { test } from "node:test";
import { expect } from "@playwright/test";

type Schema = { oneOf?: Schema[]; anyOf?: Schema[]; exclusiveMinimum?: number; $ref?: string; type?: string | string[]; required?: string[]; properties?: Record<string, Schema>; additionalProperties?: boolean; items?: Schema; enum?: unknown[]; pattern?: string; minimum?: number; maximum?: number };

function accepts(s: Schema, v: unknown): boolean {
  if (s.$ref) return accepts(s.$ref.split("/").reduce<any>((n, k) => (k === "#" ? schema : n[k]), schema), v);
  if (s.oneOf) return s.oneOf.some((x) => accepts(x, v));
  if (s.anyOf && !s.anyOf.some((x) => accepts(x, v))) return false;
  const kinds = s.type === undefined ? [] : [s.type].flat();
  const kind = Array.isArray(v) ? "array" : v === null ? "null" : typeof v;
  if (kinds.length > 0 && !kinds.includes(kind) && !(kinds.includes("integer") && Number.isInteger(v))) return false;
  if (s.enum && !s.enum.includes(v)) return false;
  if (typeof v === "string" && s.pattern && !new RegExp(s.pattern).test(v)) return false;
  if (typeof v === "number" && ((s.exclusiveMinimum !== undefined && v <= s.exclusiveMinimum) || (s.minimum !== undefined && v < s.minimum) || (s.maximum !== undefined && v > s.maximum))) return false;
  if (Array.isArray(v)) return v.every((x) => !s.items || accepts(s.items, x));
  if (kind === "object") {
    const o = v as Record<string, unknown>;
    if (s.additionalProperties === false && Object.keys(o).some((k) => !(k in (s.properties ?? {})))) return false;
    return (s.required ?? []).every((k) => o[k] !== undefined) && Object.entries(s.properties ?? {}).every(([k, p]) => o[k] === undefined || accepts(p, o[k]));
  }
  return true;
}

const t = { id: 1, name: "A", start: "2026-03-02" };
const cal = { key: "a", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 };
const shapes: [string, unknown, boolean][] = [
  ["minimal", { tasks: [t] }, true],
  ["empty tasks", { tasks: [] }, true],
  ["string id and time", { tasks: [{ ...t, id: "a", start: "2026-03-02T08:00" }] }, true],
  ["every field", { tasks: [t, { ...t, id: 2, end: "2026-03-09", duration: 2, parent: 1, progress: 50, colour: "red" }], links: [{ from: 1, to: 2, type: "SF", lag: -1 }] }, true],
  ["not an object", [t], false],
  ["null", null, false],
  ["tasks missing", { links: [] }, false],
  ["tasks an object", { tasks: t }, false],
  ["links an object", { tasks: [t], links: {} }, false],
  ["task a string", { tasks: ["x"] }, false],
  ["id missing", { tasks: [{ name: "A", start: "2026-03-02" }] }, false],
  ["id boolean", { tasks: [{ ...t, id: true }] }, false],
  ["name missing", { tasks: [{ id: 1, start: "2026-03-02" }] }, false],
  ["name number", { tasks: [{ ...t, name: 5 }] }, false],
  ["parent null", { tasks: [{ ...t, parent: null }] }, true],
  ["a folder with no start", { tasks: [{ id: 1, name: "F" }, { ...t, id: 2, parent: 1 }] }, true],
  ["parent true",{ tasks: [{ ...t, parent: true }] }, false],
  ["a constraint needs no start", { tasks: [{ id: 1, name: "A", constraint: { type: "fnlt", date: "2026-03-20" } }] }, true],
  ["a null constraint needs no start", { tasks: [{ id: 1, name: "A", constraint: null }] }, true],
  ["a constraint and a start", { tasks: [{ ...t, constraint: { type: "snet", date: "2026-03-09" } }] }, true],
  ["a constraint of another type", { tasks: [{ ...t, constraint: { type: "alap", date: "2026-03-20" } }] }, false],
  ["a constraint with no date", { tasks: [{ ...t, constraint: { type: "snet" } }] }, false],
  ["a constraint with a bad date", { tasks: [{ ...t, constraint: { type: "snet", date: "02/03/2026" } }] }, false],
  ["start slashes", { tasks: [{ ...t, start: "02/03/2026" }] }, false],
  ["end a number", { tasks: [{ ...t, end: 20260309 }] }, false],
  ["duration negative", { tasks: [{ ...t, duration: -1 }] }, false],
  ["progress over 100", { tasks: [{ ...t, progress: 101 }] }, false],
  ["progress text", { tasks: [{ ...t, progress: "50" }] }, false],
  ["link from missing", { tasks: [t], links: [{ to: 1 }] }, false],
  ["link type lower case", { tasks: [t], links: [{ from: 1, to: 1, type: "fs" }] }, false],
  ["every limit key", { tasks: [{ ...t, snet: "2026-03-03", snlt: "2026-03-09T08:00", fnet: "2026-03-04", fnlt: "2026-03-10", mso: "2026-03-05", alap: true }] }, true],
  ["a limit date that is not a date", { tasks: [{ ...t, fnlt: "next week" }] }, false],
  ["alap text", { tasks: [{ ...t, alap: "yes" }] }, false],
  ["lag text", { tasks: [t], links: [{ from: 1, to: 1, lag: "1" }] }, false],
  ["both schedule options", { tasks: [t], options: { startedLateStart: "project", unstartedRemaining: "on" } }, true],
  ["an unknown option", { tasks: [t], options: { startedLate: "p6" } }, false],
  ["an option set to false", { tasks: [t], options: { unstartedRemaining: false } }, false],
  ["options an array", { tasks: [t], options: [] }, false],
  ["a project start and a task with no start", { tasks: [{ id: 1, name: "A" }], projectStart: "2026-03-02" }, true],
  ["a project start that is text", { tasks: [t], projectStart: "soon" }, false],
  ["every progress key and a data date", { tasks: [{ ...t, actualStart: "2026-03-02", actualFinish: "2026-03-03T10:00", remaining: 0, resume: "2026-03-02" }], dataDate: "2026-03-02" }, true],
  ["calendars, a unit and a lag calendar", { tasks: [{ ...t, calendar: "a" }], calendars: [cal, { key: "b", week: [[[480, 1020]], [], [], [], [], [], []], exceptions: [{ date: "2026-12-25", working: false }] }], durationUnit: "hours", lagCalendar: { calendar: "b" } }, true],
  ["a calendar with no key", { tasks: [t], calendars: [{ workDays: [1], hoursPerDay: 8 }] }, false],
  ["workDays 8", { tasks: [t], calendars: [{ ...cal, workDays: [8] }] }, false],
  ["hoursPerDay 0", { tasks: [t], calendars: [{ ...cal, hoursPerDay: 0 }] }, false],
  ["hoursPerDay 17", { tasks: [t], calendars: [{ ...cal, hoursPerDay: 17 }] }, false],
  ["a unit of weeks", { tasks: [t], durationUnit: "weeks" }, false],
  ["a lag calendar that is a number", { tasks: [t], lagCalendar: 5 }, false],
];

test("validate_and_the_schema_agree_on_each_shape", () => {
  for (const [label, data, ok] of shapes) {
    expect(validate(data).ok, `validate: ${label}`).toBe(ok);
    expect(accepts(schema, data), `schema: ${label}`).toBe(ok);
  }
});

test("a_task_with_no_name_is_named_by_path_with_a_fix", () => {
  const caught = (() => {
    try {
      fromTasks({ tasks: [{ id: 1, start: "2026-03-02" } as never] });
    } catch (e) {
      return e;
    }
  })();
  expect(caught).toBeInstanceOf(PlainDataError);
  expect((caught as PlainDataError).issues).toEqual([{ path: "tasks[0].name", expected: "a name", got: "nothing", fix: 'Add name, for example "Site works".' }]);
  expect((caught as Error).message).toContain("tasks[0].name");
});

test("a_task_with_no_id_names_tasks_0_id", () => {
  const v = validate({ tasks: [{ name: "A", start: "2026-03-02" }] });
  expect(v.ok ? [] : v.issues.map((i) => i.path)).toEqual(["tasks[0].id"]);
});

test("two_bad_starts_give_two_issues", () => {
  const v = validate({ tasks: [{ ...t, start: "soon" }, { id: 2, name: "B", start: "2026-3-2" }] });
  expect(v.ok ? [] : v.issues.map((i) => `${i.path} ${i.got}`)).toEqual(['tasks[0].start string "soon"', 'tasks[1].start string "2026-3-2"']);
});

test("repeated_ids_and_unknown_references_are_named", () => {
  const v = validate({ tasks: [t, { ...t, parent: 9 }], links: [{ from: 1, to: 7 }] });
  expect(v.ok ? [] : v.issues.map((i) => i.path)).toEqual(["tasks[1].id", "tasks[1].parent", "links[0].to"]);
});

const faults: [string, unknown, string][] = [
  ["task_id_0", { tasks: [{ ...t, id: 0 }] }, "tasks[0].id"],
  ["a_parent_loop", { tasks: [{ ...t, parent: 2 }, { ...t, id: 2, parent: 1 }] }, "tasks[0].parent"],
  ["a_date_that_is_not_one", { tasks: [{ ...t, start: "2026-02-31" }] }, "tasks[0].start"],
  ["an_end_before_the_start", { tasks: [{ ...t, end: "2026-03-01" }] }, "tasks[0].end"],
  ["two_pins_on_one_task", { tasks: [{ ...t, mso: "2026-03-03", mfo: "2026-03-05" }] }, "tasks[0].mfo"],
  ["a_link_from_a_task_to_itself", { tasks: [t], links: [{ from: 1, to: 1 }] }, "links[0].to"],
  ["progress_and_remaining_on_one_task", { tasks: [{ ...t, actualStart: "2026-03-02", progress: 0, remaining: 1 }] }, "tasks[0].remaining"],
  ["an_actual_finish_before_the_actual_start", { tasks: [{ ...t, actualStart: "2026-03-03T10:00", actualFinish: "2026-03-03T09:00" }] }, "tasks[0].actualFinish"],
  ["a_project_start_that_is_not_a_date", { tasks: [t], projectStart: "2026-02-31" }, "projectStart"],
  ["a_data_date_that_is_not_a_date", { tasks: [t], dataDate: "2026-02-31" }, "dataDate"],
  ["an_actual_finish_before_a_date_alone_start_on_the_tasks_own_week", { tasks: [{ ...t, calendar: "pm", actualStart: "2026-03-02", actualFinish: "2026-03-02T12:00" }], calendars: [{ key: "pm", week: [[[780, 1020]], [], [], [], [], [], []] }] }, "tasks[0].actualFinish"], // a date alone starts at 13:00 there
  ["a_duration_unit_that_is_not_one", { tasks: [t], durationUnit: "weeks" }, "durationUnit"],
  ["a_calendar_with_no_key", { tasks: [t], calendars: [{ workDays: [1], hoursPerDay: 8 }] }, "calendars[0].key"],
  ["a_repeated_calendar_key", { tasks: [t], calendars: [cal, cal] }, "calendars[1].key"],
  ["both_calendar_forms", { tasks: [t], calendars: [{ ...cal, week: [[], [], [], [], [], [], [[0, 60]]] }] }, "calendars[0].week"],
  ["a_week_with_no_working_time", { tasks: [t], calendars: [{ key: "a", week: [[], [], [], [], [], [], []] }] }, "calendars[0].week"],
  ["periods_that_overlap", { tasks: [t], calendars: [{ key: "a", week: [[[480, 720], [700, 900]], [], [], [], [], [], []] }] }, "calendars[0].week"],
  ["a_workday_of_0", { tasks: [t], calendars: [{ ...cal, workDays: [0] }] }, "calendars[0].workDays"],
  ["a_workday_of_8", { tasks: [t], calendars: [{ ...cal, workDays: [8] }] }, "calendars[0].workDays"],
  ["no_workdays", { tasks: [t], calendars: [{ ...cal, workDays: [] }] }, "calendars[0].workDays"],
  ["hours_per_day_0", { tasks: [t], calendars: [{ ...cal, hoursPerDay: 0 }] }, "calendars[0].hoursPerDay"],
  ["hours_per_day_17", { tasks: [t], calendars: [{ ...cal, hoursPerDay: 17 }] }, "calendars[0].hoursPerDay"],
  ["an_exception_date_twice", { tasks: [t], calendars: [{ ...cal, exceptions: [{ date: "2026-12-25", working: false }, { date: "2026-12-25", working: true }] }] }, "calendars[0].exceptions[1].date"],
  ["an_exception_with_no_working_flag", { tasks: [t], calendars: [{ ...cal, exceptions: [{ date: "2026-12-25" }] }] }, "calendars[0].exceptions[0].working"],
  ["a_task_calendar_that_names_no_calendar", { tasks: [{ ...t, calendar: "x" }], calendars: [cal] }, "tasks[0].calendar"],
  ["a_lag_calendar_that_names_no_calendar", { tasks: [t], calendars: [cal], lagCalendar: { calendar: "x" } }, "lagCalendar"],
];
for (const [label, data, path] of faults) {
  test(`validate_refuses_${label}_and_fromTasks_throws_a_PlainDataError`, () => {
    const v = validate(data);
    expect(v.ok ? [] : v.issues.map((i) => i.path)).toEqual([path]);
    expect(() => fromTasks(data as never)).toThrow(PlainDataError);
  });
}

test("a_top_level_folder_with_parent_null_and_no_start_a_nested_folder_and_leaves_validate_and_mount", () => {
  const data = { tasks: [
    { id: "f", name: "Folder", parent: null },
    { id: "g", name: "Nested", parent: "f" },
    { id: 1, name: "A", start: "2026-03-02", duration: 2, parent: "g" },
    { id: 2, name: "B", start: "2026-03-04", duration: 1, parent: "f" },
    { id: 3, name: "C", start: "2026-03-02", duration: 1, parent: null },
  ] };
  expect(validate(data)).toEqual({ ok: true });
  const tasks = fromTasks(data).document.tasks;
  expect(tasks.map((x) => [x.code, x.outline_level])).toEqual([["f", 1], ["g", 2], ["1", 3], ["2", 2], ["3", 1]]);
  expect(tasks.map((x) => x.parent)).toEqual([null, 0, 1, 0, null]);
});

test("parent_null_is_a_root_even_when_a_task_has_the_id_null_and_a_leaf_still_needs_a_start", () => {
  const roots = fromTasks({ tasks: [{ id: "null", name: "N", start: "2026-03-02" }, { id: 1, name: "A", start: "2026-03-02", parent: null }] }).document.tasks;
  expect(roots.map((x) => x.parent), "null names no task, not the task whose id is the text null").toEqual([null, null]);
  const missing = (data: unknown) => { const v = validate(data); return v.ok ? [] : v.issues.map((i) => i.path); };
  expect(missing({ tasks: [{ id: 1, name: "A" }] }), "a leaf").toEqual(["tasks[0].start"]);
  expect(missing({ tasks: [{ id: 1, name: "A", parent: null }] }), "a leaf with parent null").toEqual(["tasks[0].start"]);
  expect(missing({ tasks: [{ id: 1, name: "A", start: "soon" }, { ...t, id: 2, parent: 1 }] }), "a folder's own start is still checked when given").toEqual(["tasks[0].start"]);
});

test("an_end_before_the_start_that_fromTasks_never_reads_still_validates_and_mounts", () => {
  const early = { ...t, end: "2026-03-01" };
  for (const data of [{ tasks: [{ ...t, start: "2026-03-02T00:00", end: "2026-03-02" }] },{ tasks: [{ ...early, duration: 2 }] }, { tasks: [{ ...early, type: "milestone" }] }, { tasks: [early, { ...t, id: 2, parent: 1 }] }]) {
    expect(validate(data)).toEqual({ ok: true }); // a summary's own dates, a duration, a milestone
    expect(() => fromTasks(data as never)).not.toThrow();
  }
});

test("an_actual_finish_remaining_or_resume_with_no_actual_start_is_refused_and_a_date_alone_finish_on_the_start_day_is_not_before_it", () => {
  const v = validate({ tasks: [{ ...t, actualFinish: "2026-03-03", remaining: 1, resume: "2026-03-03" }] });
  expect(v.ok ? [] : v.issues.map((i) => i.path)).toEqual(["tasks[0].actualFinish", "tasks[0].remaining", "tasks[0].resume"]);
  expect(validate({ tasks: [{ ...t, actualStart: "2026-03-03T08:00", actualFinish: "2026-03-03" }] })).toEqual({ ok: true }); // a finish date alone is 17:00
  expect(validate({ tasks: [{ ...t, calendar: "am", actualStart: "2026-03-02", actualFinish: "2026-03-02T07:00" }], calendars: [{ key: "am", week: [[[360, 720]], [], [], [], [], [], []] }] })).toEqual({ ok: true }); // a date alone starts at 06:00 on that week, not at the standard 08:00
  expect(() => fromTasks({ tasks: [{ ...t, actualStart: "2026-03-03", progress: 0, remaining: 1 }] })).toThrow("expected remaining or progress, not both"); // the message names both
});

test("issues_stop_at_fifty", () => {
  const v = validate({ tasks: Array.from({ length: 80 }, () => ({})) });
  expect(v.ok ? 0 : v.issues.length).toBe(50);
});

test("holes_in_a_sparse_array_are_skipped_not_reported", () => {
  expect(validate({ tasks: new Array(1) })).toEqual({ ok: true });
  expect(validate({ tasks: [t, , { ...t, id: 2 }], links: [, { from: 1, to: 2 }] })).toEqual({ ok: true });
  const v = validate({ tasks: [, {}], links: [, {}] });
  expect(v.ok ? [] : v.issues.map((i) => i.path)).toEqual(["tasks[1].id", "tasks[1].name", "tasks[1].start", "links[1].from", "links[1].to"]);
});

test("the_date_check_agrees_with_a_pattern_and_date_parse_on_days_that_do_not_exist_and_dates_written_wrong", () => {
  const pattern = /^(?!00)\d{4}-\d{2}-\d{2}(T([01]\d|2[0-3]):[0-5]\d)?$/;
  const real = (day: string) => { const ms = Date.parse(day); return ms === ms && new Date(ms).toISOString().startsWith(day); };
  const reference = (s: string) => pattern.test(s) && real(s.slice(0, 10));
  const dates = [
    "2026-02-28", "2026-02-29", "2026-02-30", "2026-04-30", "2026-04-31", "2026-12-31", "2026-13-01", "2026-00-10", "2026-01-00", "2026-01-32",
    "2023-02-29", "2024-02-29", "2000-02-29", "1900-02-29", "2100-02-29", "0099-12-31", "0100-01-01", "0000-01-01", "9999-12-31",
    "2026-2-03", "2026-02-3", "26-02-03", "2026-02-03T8:00", "2026-02-03T08:0", "2026/02/03", "2026-02-03 08:00",
    "2026-02-03T00:00", "2026-02-03T23:59", "2026-02-03T24:00", "2026-02-03T08:60", "2026-02-30T08:00",
    "2026-02-03x", "2026-02-03T08:00Z", "2026-02-03T08:00:00", "2026-02-03 ", " 2026-02-03", "2026-02-03\n", "", "２０２６-02-03",
  ];
  const agree = dates.map((s) => [s, validate({ tasks: [{ id: 1, name: "A", start: s }] }).ok]);
  expect(agree).toEqual(dates.map((s) => [s, reference(s)]));
  expect(agree.filter(([, ok]) => ok).length).toBe(9);
});

test("each_task_and_link_field_with_a_wrong_value_is_named_by_its_own_path", () => {
  const paths = (data: unknown) => { const v = validate(data); return v.ok ? [] : v.issues.map((i) => i.path); };
  const unchecked = ["color", "label"];
  for (const name of Object.keys(schema.$defs.task.properties).filter((n) => !unchecked.includes(n))) expect(paths({ tasks: [{ ...t, [name]: [] }] })).toContain(`tasks[0].${name}`);
  for (const name of Object.keys(schema.$defs.link.properties)) expect(paths({ tasks: [t, { ...t, id: 2 }], links: [{ from: 1, to: 2, [name]: [] }] })).toContain(`links[0].${name}`);
});

test("a_number_id_and_the_same_id_as_text_repeat_in_either_order", () => {
  const repeats = (...list: (string | number)[]) => { const v = validate({ tasks: list.map((id) => ({ ...t, id })) }); return v.ok ? [] : v.issues.map((i) => `${i.path} ${i.got}`); };
  expect(repeats(1, "1")).toEqual(['tasks[1].id string "1", also tasks[0].id']);
  expect(repeats("1", 1)).toEqual(["tasks[1].id number 1, also tasks[0].id"]);
  expect(repeats("a", 2, 2)).toEqual(["tasks[2].id number 2, also tasks[1].id"]);
});

test("an_unknown_option_member_or_value_is_refused_by_name_and_remaining_alone_needs_the_option_on", () => {
  const paths = (options: unknown) => { const v = validate({ tasks: [t], options }); return v.ok ? [] : v.issues.map((i) => i.path); };
  expect(paths({ startedLate: "p6", unstartedRemaining: false, startedLateStart: "actual" })).toEqual(["options.startedLate", "options.unstartedRemaining", "options.startedLateStart"]);
  expect(paths([])).toEqual(["options"]);
  expect(() => fromTasks({ tasks: [t], options: { unstartedRemaining: "yes" } as never })).toThrow('options.unstartedRemaining: expected one of "off", "on"');
  const alone = { tasks: [{ ...t, remaining: 3 }] };
  expect(validate({ ...alone, options: { unstartedRemaining: "on" } })).toEqual({ ok: true });
  expect(validate({ ...alone, options: { unstartedRemaining: "off" } }).ok).toBe(false);
});
