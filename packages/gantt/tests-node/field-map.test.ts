// The field map (decision 0022, rule 8.4): the three rejections, and where a host's names are read and written.
import { readFields } from "../src/field-map.ts";
import { hostData } from "../src/host-data.ts";
import type { Change } from "../src/events.js";
import type { Fields } from "../src/gantt-types.js";
import { test } from "node:test";
import { expect } from "@playwright/test";

const tasks = { name: "title", start: "plannedStart", duration: "days", parent: "parentId", key: "id" };
const map = (fields: Fields, warnings: string[] = []) => readFields(fields, (text) => warnings.push(text));
const host = { id: "b", title: "Pour", plannedStart: "2026-03-02", days: 3, parentId: "a", owner: "sam" };

test("an entry for a field the chart does not have, one host name for two fields and a host name that is another field's own are refused, each naming its fields", () => {
  expect(() => map({ tasks: { titel: "title" } })).toThrow(/fields\.tasks.*"titel"/);
  expect(() => map({ links: { name: "title" } })).toThrow(/fields\.links.*"name"/);
  expect(() => map({ tasks: { name: "label", progress: "label" } })).toThrow(/"label".*"name".*"progress"/);
  expect(() => map({ tasks: { name: "start" } })).toThrow(/fields\.tasks\.name is "start".*add one for "start"/);
  expect(() => map({ tasks: { name: "start", start: "name" } })).not.toThrow();
  expect(() => map({ tasks: { key: "id" }, links: { key: "id" } })).not.toThrow();
});

test("a map renames the rows a host gives, update's records, and the members of a change's set and was", () => {
  const names = map({ tasks, links: { from: "source", to: "target" } });
  expect(names.tasks.rows([host])).toEqual([{ key: "b", id: "b", name: "Pour", start: "2026-03-02", duration: 3, parent: "a", owner: "sam" }]);
  expect(names.tasks.toChart({ id: "b", days: 4 })).toEqual({ key: "b", duration: 4 });
  expect(names.links.rows([{ source: "a", target: "b", type: "SS" }])).toEqual([{ from: "a", to: "b", type: "SS" }]);
  const change: Change = { record: "task", op: "update", key: "b", task: 3, set: { duration: 4, name: "Pour 2" }, was: { duration: 3, name: "Pour", milestone: true } };
  expect(names.changes([change])).toEqual([{ ...change, set: { days: 4, title: "Pour 2" }, was: { days: 3, title: "Pour", milestone: true } }]);
  expect(names.changes([{ record: "link", op: "update", key: "l", link: 0, set: { from: "a" }, was: { from: "c" } }])[0]).toMatchObject({ set: { source: "a" }, was: { source: "c" } });
});

test("key: id makes the host's id the row's key, and a create carries it as id", () => {
  const names = map({ tasks: { key: "id" } });
  expect(names.tasks.rows([{ id: "t1", name: "A", start: "2026-03-02" }])).toEqual([{ key: "t1", id: "t1", name: "A", start: "2026-03-02" }]);
  const made: Change = { record: "task", op: "create", key: "t9", set: { key: "t9", id: "x", name: "B" } };
  expect(names.changes([made])).toEqual([{ ...made, set: { id: "t9", name: "B" } }]);
});

test("a field with no entry keeps its name, the chart's own name of a mapped field is the host's custom field, and a value that is no record passes", () => {
  const names = map({ tasks: { name: "title" } });
  expect(names.tasks.toChart({ key: "k", title: "A", name: "mine", progress: 5 })).toEqual({ key: "k", name: "A", progress: 5 });
  expect(names.tasks.toChart(null)).toBeNull();
  expect(names.tasks.toChart("x")).toBe("x");
  const rows = [{ name: "A" }];
  expect(map({}).tasks.rows(rows)).toBe(rows);
});

test("a change's own members and a schedule change are not renamed", () => {
  const names = map({ tasks: { key: "uuid", name: "title", milestone: "op" } });
  const change: Change = { record: "task", op: "delete", key: "k", task: 1, was: { key: "k", name: "A", milestone: "start" } };
  expect(names.changes([change])[0]).toEqual({ ...change, was: { uuid: "k", title: "A", op: "start" } });
  const schedule: Change = { record: "schedule", op: "update", key: null, set: { name: "Plan" }, was: { name: "Old" } };
  expect(names.changes([schedule])).toEqual([schedule]);
});

test("setData of host-named rows sets the changed members and nothing else", () => {
  const names = map({ tasks });
  const rows = (title: string, days: number) => [{ id: "a", title: "Site", plannedStart: "2026-03-02", days: 1 }, { ...host, title, days }];
  const sent: unknown[] = [];
  const setData = hostData(names.data({ tasks: rows("Pour", 3) }) as never, 0);
  const answer = setData(names.data({ tasks: rows("Pour 2", 3) }) as never, 0, (records) => (sent.push(records), { ok: true, updated: records.length }));
  expect(answer).toEqual({ ok: true, updated: 1 });
  expect(sent).toEqual([[{ key: "b", name: "Pour 2" }]]);
});

test("one warning when no first row carries a mapped name, and none after it", () => {
  const warnings: string[] = [];
  const names = map({ tasks: { name: "titel" } }, warnings);
  names.tasks.rows([]);
  names.tasks.rows([{ name: "A" }]);
  names.tasks.rows([{ name: "A" }]);
  expect(warnings).toHaveLength(1);
  expect(warnings[0]).toContain("titel");
  map({ tasks: { name: "title" } }, warnings).tasks.rows([{ title: "A" }]);
  expect(warnings).toHaveLength(1);
});
