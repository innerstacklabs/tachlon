// `toPlain` (gantt#2077): a document back as plain tasks and links, the start of each task given by the caller. The expected
// values are written out here, not computed by the adapter: a working day is 480 minutes, ids come back as text.
import { fromTasks, type PlainLink, type PlainTask } from "../src/adapters/plain.ts";
import { toPlain } from "../src/adapters/to-plain.ts";
import { test } from "node:test";
import { expect } from "@playwright/test";

const tasks: PlainTask[] = [
  { id: "site", name: "Site works", start: "2026-03-02" },
  { id: "strip", name: "Strip out", start: "2026-03-02T08:00", duration: 3, parent: "site", progress: 50, key: "ext-1", color: "#aa0000", label: "Strip" },
  { id: "walls", name: "Partitions", start: "2026-03-05", duration: 4, parent: "site", progress: 25 },
  { id: 7, name: "Handover", start: "2026-03-12", duration: 0, progress: 100 },
];
const links: PlainLink[] = [{ from: "strip", to: "walls", type: "SS", lag: 1 }, { from: "walls", to: 7 }];
const starts = ["2026-03-02 08:00", "2026-03-02 08:00", "2026-03-05 08:00", "2026-03-12 08:00"];

test("plain_in_document_out_is_plain_again", () => {
  const out = toPlain(fromTasks({ tasks, links }).document, starts);
  expect(out.tasks).toEqual([
    { id: "site", name: "Site works", start: "2026-03-02T08:00", duration: 0, progress: 0 },
    { id: "strip", name: "Strip out", start: "2026-03-02T08:00", duration: 3, parent: "site", progress: 50, label: "Strip", color: "#aa0000", key: "ext-1" },
    { id: "walls", name: "Partitions", start: "2026-03-05T08:00", duration: 4, parent: "site", progress: 25 },
    { id: "7", name: "Handover", start: "2026-03-12T08:00", duration: 0, progress: 100 },
  ]);
  expect(out.links).toEqual([{ from: "strip", to: "walls", type: "SS", lag: 1 }, { from: "walls", to: "7", type: "FS", lag: 0 }]);
});

test("a_link_key_comes_back_and_a_link_without_one_has_none", () => {
  const { document } = fromTasks({
    tasks: [{ id: "a", name: "A", start: "2026-03-02", duration: 1 }, { id: "b", name: "B", start: "2026-03-02", duration: 1 }, { id: "c", name: "C", start: "2026-03-02", duration: 1 }],
    links: [{ from: "a", to: "b", key: "l-1" }, { from: "b", to: "c" }],
  });
  const out = toPlain(document, ["2026-03-02 08:00", "2026-03-03 08:00", "2026-03-04 08:00"]);
  expect(out.links[0]!.key).toBe("l-1");
  expect(out.links[1]).not.toHaveProperty("key");
  expect(fromTasks(out).document.links.map((link) => link.key)).toEqual(["l-1", undefined]);
});

test("a_linked_task_takes_the_date_passed_in_not_its_own_start", () => {
  const { document } = fromTasks({ tasks: [{ id: "a", name: "A", start: "2026-03-02", duration: 3 }, { id: "b", name: "B", start: "2026-03-02", duration: 2 }], links: [{ from: "a", to: "b" }] });
  const out = toPlain(document, ["2026-03-02 08:00", "2026-03-05 08:00"]);
  expect(out.tasks.map((task) => task.start)).toEqual(["2026-03-02T08:00", "2026-03-05T08:00"]);
  expect(out.tasks.map((task) => task.snet)).toEqual([undefined, "2026-03-02T00:00"]);
  expect(out.leftOut).toEqual([]);
});

test("limits_come_back_as_the_plain_keys_fromTasks_reads", () => {
  const limited: PlainTask[] = [
    { id: "a", name: "A", start: "2026-03-02", duration: 2, fnlt: "2026-03-06" },
    { id: "b", name: "B", start: "2026-03-03", duration: 1, snlt: "2026-03-10T09:00", fnet: "2026-03-04T12:00" },
    { id: "c", name: "C", start: "2026-03-02", duration: 1, mso: "2026-03-09" },
    { id: "d", name: "D", start: "2026-03-02", duration: 1, alap: true },
    { id: "e", name: "E", start: "2026-03-02", duration: 0, milestone: "finish" },
  ];
  const first = fromTasks({ tasks: limited }).document;
  const out = toPlain(first, ["2026-03-02 08:00", "2026-03-05 08:00", "2026-03-09 08:00", "2026-03-02 08:00", "2026-03-02 08:00"]);
  expect(out.tasks.map((task) => task.milestone)).toEqual([undefined, undefined, undefined, undefined, "finish"]);
  expect(out.tasks.map(({ snet, snlt, fnet, fnlt, mso, mfo, alap }) => ({ snet, snlt, fnet, fnlt, mso, mfo, alap }))).toEqual([
    { snet: undefined, snlt: undefined, fnet: undefined, fnlt: "2026-03-06T17:00", mso: undefined, mfo: undefined, alap: undefined },
    { snet: "2026-03-03T00:00", snlt: "2026-03-10T09:00", fnet: "2026-03-04T12:00", fnlt: undefined, mso: undefined, mfo: undefined, alap: undefined },
    { snet: "2026-03-02T00:00", snlt: undefined, fnet: undefined, fnlt: undefined, mso: "2026-03-09T08:00", mfo: undefined, alap: undefined },
    { snet: undefined, snlt: undefined, fnet: undefined, fnlt: undefined, mso: undefined, mfo: undefined, alap: true },
    { snet: undefined, snlt: undefined, fnet: undefined, fnlt: undefined, mso: undefined, mfo: undefined, alap: undefined },
  ]);
  expect(out.leftOut).toEqual([]);
  const again = fromTasks({ tasks: out.tasks }).document;
  expect(again.origin).toBe(first.origin);
  expect(again.tasks.map((task) => task.limit)).toEqual(first.tasks.map((task) => task.limit));
});

test("actuals_remaining_resume_and_the_data_date_come_back_and_progress_is_left_out_beside_remaining", () => {
  const given: PlainTask[] = [
    { id: "a", name: "A", start: "2026-03-02", duration: 4, actualStart: "2026-03-02T08:00", remaining: 1.5, resume: "2026-03-05" },
    { id: "b", name: "B", start: "2026-03-02", duration: 1, actualStart: "2026-03-02", actualFinish: "2026-03-02", progress: 100 },
  ];
  const first = fromTasks({ tasks: given, dataDate: "2026-03-04" }).document;
  const out = toPlain(first, ["2026-03-02 00:00", "2026-03-02 00:00"]);
  expect(out.tasks.map(({ actualStart, actualFinish, remaining, resume, progress }) => ({ actualStart, actualFinish, remaining, resume, progress }))).toEqual([
    { actualStart: "2026-03-02T08:00", actualFinish: undefined, remaining: 1.5, resume: "2026-03-05T00:00", progress: undefined },
    { actualStart: "2026-03-02T08:00", actualFinish: "2026-03-02T17:00", remaining: undefined, resume: undefined, progress: 100 },
  ]);
  expect(out.dataDate).toBe("2026-03-04T00:00");
  expect(out.leftOut).toEqual([]);
  expect(fromTasks(out).document).toEqual(first);
});

test("a_started_task_with_no_finish_and_no_remaining_comes_back_without_a_progress_that_would_set_the_remaining_work", () => {
  const given: PlainTask[] = [{ id: "a", name: "A", start: "2026-03-02", duration: 4, progress: 40 }];
  const unstarted = fromTasks({ tasks: given }).document;
  const first = { ...unstarted, tasks: unstarted.tasks.map((task) => ({ ...task, actual_start: 480 })) };
  const out = toPlain(first, ["2026-03-02 00:00"]);
  expect(out.tasks.map(({ actualStart, remaining, progress }) => ({ actualStart, remaining, progress }))).toEqual([{ actualStart: "2026-03-02T08:00", remaining: undefined, progress: undefined }]);
  const again = fromTasks(out).document.tasks[0]!;
  expect([again.actual_start, again.actual_finish, again.remaining], "it mounts again with the same dates").toEqual([480, null, null]);
});

test("the_options_come_back_and_a_schedule_without_them_writes_none", () => {
  const options = { unstartedRemaining: "on" } as const;
  expect(toPlain(fromTasks({ tasks, options }).document, starts).options).toEqual(options);
  expect(toPlain(fromTasks({ tasks }).document, starts)).not.toHaveProperty("options");
  const unstarted = fromTasks({ tasks: [{ id: "a", name: "A", start: "2027-01-04", duration: 5, remaining: 3 }], options }).document;
  expect(fromTasks(toPlain(unstarted, ["2027-01-04 00:00"])).document).toEqual(unstarted);
});

test("ids_are_places_when_the_activity_ids_are_not_all_there", () => {
  const { document } = fromTasks({ tasks: [{ id: "a", name: "A", start: "2026-03-02", duration: 1 }, { id: "b", name: "B", start: "2026-03-02", duration: 1, parent: "a" }] });
  document.tasks[1]!.code = null;
  const out = toPlain(document, starts);
  expect(out.tasks.map((task) => [task.id, task.parent])).toEqual([["1", undefined], ["2", "1"]]);
  expect(out.leftOut.at(-1)).toBe("Not every task has a different activity ID, so each id is the task's place in the list, from 1.");
});

// gantt#2081: plain data now takes calendars, but `toPlain` writes none, so a schedule on a calendar other than the Monday to Friday week is named in `leftOut` even with a single calendar.
test("one_calendar_that_is_not_the_standard_week_is_left_out_by_name_and_the_standard_week_is_not", () => {
  const read = (calendars?: object[]) => toPlain(fromTasks({ tasks: [{ id: "a", name: "A", start: "2026-03-02", duration: 1 }], calendars } as never).document, ["2026-03-02 08:00"]).leftOut;
  expect(read()).toEqual([]);
  expect(read([{ key: "ten", workDays: [1, 2, 3, 4], hoursPerDay: 10 }])).toEqual([expect.stringMatching(/^The schedule has a calendar other than the one Monday to Friday week/)]);
});
