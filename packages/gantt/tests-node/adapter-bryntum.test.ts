// The Bryntum Gantt adapter (gs#1098): a developer who runs Bryntum Gantt today reads its task JSON into the
// document (decision 0010) with `fromBryntum` and feeds that to the chart through `load` (the data API, gs#1084).
// The fixture is a Bryntum load response (tasks with children, dependencies, and the tables the adapter leaves out).
// The dates below are plain calendar arithmetic on the document's one calendar: Monday to Friday, 08:00 to 12:00
// and 13:00 to 17:00, so a day is 480 working minutes.
import { readFileSync } from "node:fs";
import { fromBryntum } from "../src/adapters/bryntum.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const fixture: unknown = JSON.parse(readFileSync(new URL("../tests/fixtures/bryntum-gantt.json", import.meta.url), "utf8"));

/** The adapter's document for the fixture, or a failure naming why there is none. */
function read(data: unknown = fixture) {
  const result = fromBryntum(data);
  if (!result.ok) throw new Error(`the fixture was refused: ${result.error}`);
  return result;
}

test("tasks_keep_their_names_outline_durations_and_progress", () => {
  const { document } = read();
  const rows = document.tasks.map((t) => [t.name, t.parent, t.outline_level, t.duration, t.progress, t.code]);
  expect(rows).toEqual([
    ["Launch", null, 1, 0, 0, "1"],
    ["Design", 0, 2, 3 * 480, 100, "2"],
    ["Build", 0, 2, 10 * 480, 40, "3"],
    ["Review", 0, 2, 4 * 60, 0, "4"],
    ["Release", 0, 2, 0, 0, "5"],
    ["Operate", null, 1, 5 * 480, 0, "6"],
    ["Documentation", null, 1, 2 * 480, 0, "7"],
    ["Sign-off", null, 1, 480, 0, "8"],
  ]);
});

test("start_dates_become_the_earliest_start_only_where_the_data_fixes_one", () => {
  const { document } = read();
  // Origin is Monday 2026-01-05. Design has no predecessor: its start is kept. Build starts after Design: no start.
  // Operate has a start-no-earlier-than constraint on 2026-02-02, 28 days on.
  expect(document.tasks.map((t) => t.snet)).toEqual([null, 0, null, null, null, 28 * 1440, null, null]);
  expect(document.origin).toBe(Date.UTC(2026, 0, 5) / 86_400_000);
});

test("links_keep_their_type_and_lag", () => {
  const { document } = read();
  expect(document.links.map((l) => [l.pred, l.succ, l.kind, l.lag])).toEqual([
    [1, 2, "FS", 0],
    [2, 3, "FS", 480],
    [3, 4, "FS", 0],
    [4, 5, "FS", 0],
    [1, 6, "SS", 960],
    [2, 7, "FF", 0],
    [5, 6, "SF", 0],
  ]);
});

test("what_the_adapter_leaves_out_is_listed", () => {
  const { leftOut, document } = read();
  const text = leftOut.join("\n");
  expect(text).toMatch(/Dependency 8 .*parent task/);
  expect(text).toMatch(/Dependency 9 .*task 99/);
  expect(text).toMatch(/Dependency 10 .*repeats/);
  expect(text).toMatch(/Resources and assignments/);
  expect(text).toMatch(/Calendars/);
  expect(document.notes, "the chart's import-notes button shows the same lines").toEqual(leftOut);
});

test("flat_rows_with_parent_ids_read_like_nested_children", () => {
  const flat = {
    tasks: [
      { id: "a", name: "Top" },
      { id: "b", name: "Under", parentId: "a", startDate: "2026-01-05", duration: 2 },
    ],
    dependencies: [],
  };
  const { document } = read(flat);
  expect(document.tasks.map((t) => [t.name, t.parent, t.outline_level, t.duration])).toEqual([
    ["Top", null, 1, 0],
    ["Under", 0, 2, 960],
  ]);
});

test("each_constraint_type_gives_the_date_key_the_chart_holds_it_by", () => {
  // One task per type, no start date. The earliest date, Tuesday 2026-01-06, puts the origin on Monday 2026-01-05.
  const types = ["startnoearlierthan", "startnolaterthan", "finishnoearlierthan", "finishnolaterthan", "muststarton", "mustfinishon"];
  const tasks = types.map((constraintType, i) => ({ id: i + 1, name: constraintType, duration: 1, constraintType, constraintDate: `2026-01-${String(6 + i).padStart(2, "0")}T00:00:00` }));
  const { document, leftOut } = read({ tasks, dependencies: [] });
  const day = (i: number) => (1 + i) * 1440;
  expect(document.tasks.map((t) => [t.snet, t.limit])).toEqual([
    [day(0), undefined],
    [null, { snlt: day(1) }],
    [null, { fnet: day(2) }],
    [null, { fnlt: day(3) }],
    [null, { pin_start: day(4) }],
    [null, { pin_finish: day(5) }],
  ]);
  expect(leftOut.join("\n"), "all six are read, so none is listed as not read").not.toMatch(/constraint/);
});

test("a_constraint_with_no_date_is_still_listed", () => {
  const tasks = [{ id: 2, name: "undated", duration: 1, constraintType: "mustfinishon" }];
  const { document, leftOut } = read({ tasks, dependencies: [] });
  expect(document.tasks.map((t) => t.limit)).toEqual([undefined]);
  expect(leftOut.join("\n")).toMatch(/1 task\(s\) have a constraint that is not read/);
});

test("as_late_as_possible_gives_alap_with_or_without_a_date_and_is_not_listed", () => {
  const tasks = [
    { id: 1, name: "dated", duration: 1, constraintType: "AsLateAsPossible", constraintDate: "2026-01-06" },
    { id: 2, name: "undated", duration: 1, constraintType: "aslateaspossible" },
  ];
  const { document, leftOut } = read({ tasks, dependencies: [] });
  expect(document.tasks.map((t) => [t.snet, t.limit])).toEqual([[null, { alap: true }], [null, { alap: true }]]);
  expect(leftOut.join("\n"), "both are read, so none is listed as not read").not.toMatch(/constraint/);
});

test("data_that_is_not_a_task_list_is_a_result_not_a_throw", () => {
  expect(fromBryntum("nonsense")).toMatchObject({ ok: false, error: expect.any(String) });
  expect(fromBryntum({ success: true })).toMatchObject({ ok: false, error: expect.stringContaining("tasks") });
});
