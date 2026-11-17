// The DHTMLX data adapter (gs#1097): the fixture is DHTMLX Gantt JSON (tests/fixtures/dhtmlx.json). It loads into the chart as
// a document and the chart gives back its tasks, links, dates and durations. What the spec expects is worked out here from the
// fixture's own text, not by calling the adapter, so a mistake in the adapter cannot agree with itself.
import { readFileSync } from "node:fs";
import { fromDhtmlx } from "../src/adapters/dhtmlx.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

interface FixtureTask {
  id: number;
  text: string;
  type?: string;
  start_date?: string;
  duration?: number;
  progress?: number;
  parent?: number;
}
interface FixtureLink {
  source: number;
  target: number;
  type: string;
  lag?: number;
}
const fixture = JSON.parse(readFileSync(new URL("../tests/fixtures/dhtmlx.json", import.meta.url), "utf8")) as { data: FixtureTask[]; links: FixtureLink[] };

test("the_adapter_writes_a_document_for_the_fixture_and_says_what_it_left_out", () => {
  const { document, leftOut } = fromDhtmlx(fixture, { name: "fit-out" });
  expect(document).toMatchObject({ format: "gantt", version: 1, name: "fit-out", baseline: null, source: null });
  expect(document.tasks).toHaveLength(fixture.data.length);
  expect(document.links, "the link to a summary is not carried").toHaveLength(fixture.links.length - 1);
  expect(new Date(document.origin * 86_400_000).getUTCDay(), "the origin is a Monday").toBe(1);
  expect(document.origin * 1440).toBeLessThanOrEqual(Date.UTC(2027, 2, 1) / 60_000);
  expect(document.notes).toEqual(leftOut);
  expect(leftOut.join("\n")).toContain('field "color" is not read (2)');
  expect(leftOut.join("\n")).toContain('field "resource_id" is not read (1)');
  expect(leftOut.join("\n")).toContain('"collections"');
  expect(leftOut.join("\n")).toContain("1 link touches a summary task");
});

test("data_that_cannot_be_read_is_an_error_that_names_the_task", () => {
  expect(() => fromDhtmlx({ data: [{ id: 4, start_date: "next week" }] })).toThrow(/task 4: start_date "next week" is not a date/);
  expect(() => fromDhtmlx({ data: [{ id: 4, parent: 9 }] })).toThrow(/task 4 has parent 9/);
  expect(() => fromDhtmlx({ data: [{ id: 4 }, { id: 4 }] })).toThrow(/task id 4 is used twice/);
  expect(() => fromDhtmlx({ data: [{ id: 1 }], links: [{ id: 3, source: 1, target: 2 }] })).toThrow(/link 3 names a task that is not in data/);
  expect(() => fromDhtmlx({ data: [{ id: 1, parent: 2 }, { id: 2, parent: 1 }] })).toThrow(/loop/);
  expect(() => fromDhtmlx({ tasks: [] })).toThrow(/"data" array/);
});

test("dates_are_read_in_both_the_dhtmlx_formats", () => {
  const dmy = fromDhtmlx({ data: [{ id: 1, start_date: "05-01-2026 09:30", duration: 2 }] }).document;
  const iso = fromDhtmlx({ data: [{ id: 1, start_date: "2026-01-05 09:30:00", duration: 2 }] }).document;
  expect(dmy.tasks[0]!.snet).toBe(570);
  expect(iso.tasks[0]).toEqual(dmy.tasks[0]);
  const hours = fromDhtmlx({ data: [{ id: 1, start_date: "2026-01-05", duration: 3 }], links: [] }, { durationUnit: "hour" }).document;
  expect(hours.tasks[0]!.duration).toBe(180);
});

test("a_constraint_type_and_date_open_as_the_chart_holds_them", () => {
  // The origin is Monday 2026-01-05, so a constraint date is its days from the 5th at 1440 minutes a day, plus 09:30 (570). `06-01-2026 09:30` is the form DHTMLX 10.0.3 `serialize()` writes.
  const task = (id: number, constraint_type: string, constraint_date?: string, more: object = {}) => ({ id, duration: 1, constraint_type, constraint_date, ...more });
  const { document, leftOut } = fromDhtmlx({
    data: [
      task(1, "asap", undefined, { start_date: "05-01-2026" }),
      task(2, "alap"),
      task(3, "snet", "06-01-2026 09:30", { start_date: "05-01-2026 08:00" }),
      task(4, "snlt", "07-01-2026 09:30"),
      task(5, "fnet", "08-01-2026 09:30"),
      task(6, "fnlt", "2026-01-09 09:30"),
      task(7, "mso", "12-01-2026 09:30"),
      task(8, "mfo", "13-01-2026 09:30"),
      task(9, "snlt"),
      task(10, "bogus", "07-01-2026 09:30"),
      task(11, "snet", "06-01-2026 09:30"),
      { id: 12, parent: 11, duration: 1 },
    ],
  });
  expect(document.tasks.map((t) => t.limit)).toEqual([undefined, { alap: true }, undefined, { snlt: 3450 }, { fnet: 4890 }, { fnlt: 6330 }, { pin_start: 10650 }, { pin_finish: 12090 }, undefined, undefined, undefined, undefined]);
  expect(document.tasks[0]!.snet, "asap adds nothing to start_date").toBe(0);
  expect(document.tasks[2]!.snet, "snet is the start floor in place of start_date").toBe(2010);
  const text = leftOut.join("\n");
  expect(text).not.toContain("constraint_");
  expect(text).toContain("2 tasks have a constraint with no date or of a type that is not known");
  expect(text).toContain("progress and constraint of 1 summary task");
  expect(() => fromDhtmlx({ data: [task(4, "snlt", "soon")] })).toThrow(/task 4: constraint_date "soon" is not a date/);
});

test("planned_dates_become_the_baseline_with_no_float_and_a_one_sided_task_is_counted", () => {
  // The origin is Monday 2026-01-05, so a planned date is its days from the 5th at 1440 minutes a day, plus its time of day (08:00 is 480, 12:00 720, 17:00 1020).
  const planned = (id: number, planned_start?: string, planned_end?: string, more: object = {}) => ({ id, start_date: "05-01-2026", duration: 1, planned_start, planned_end, ...more });
  const { document, leftOut } = fromDhtmlx({
    data: [planned(1, "2026-01-05 08:00", "07-01-2026 17:00"), planned(2, "06-01-2026 08:00"), planned(3, "05-01-2026", "09-01-2026"), planned(4, "12-01-2026 08:00", "14-01-2026 12:00", { parent: 3 })],
  });
  expect(document.baseline, "a summary and a one-sided task have no entry; the finish is the latest planned finish").toEqual({ data_date: null, project_finish: 13680, tasks: [[480, 3900, null], null, null, [10560, 13680, null]] });
  const text = leftOut.join("\n");
  expect(text).toContain("1 task has only one of planned_start and planned_end");
  expect(text).not.toContain('field "planned_');
  expect(fromDhtmlx({ data: [{ id: 1, start_date: "05-01-2026", duration: 1 }] }).document.baseline, "no planned dates, no baseline").toBeNull();
  expect(() => fromDhtmlx({ data: [planned(4, "05-01-2026", "soon")] })).toThrow(/task 4: planned_end "soon" is not a date/);
});

test("a_dhtmlx_color_of_the_form_rrggbb_reaches_the_document_and_any_other_value_stays_in_leftOut", () => {
  const read = (task: object) => fromDhtmlx({ data: [{ id: 1, start_date: "05-01-2026", duration: 1, ...task }] });
  expect(read({ color: "#2e7d32" }).document.tasks[0]).toMatchObject({ color: "#2e7d32" });
  expect(read({ color: "#2e7d32" }).leftOut).toEqual([]);
  expect(read({ color: "red" }).document.tasks[0]).not.toHaveProperty("color");
  expect(read({ color: "red" }).leftOut).toEqual(['DHTMLX task field "color" is not read (1).']);
  expect(read({ label: "x" }).leftOut, "DHTMLX has no label: its text is the name").toEqual(['DHTMLX task field "label" is not read (1).']);
});
