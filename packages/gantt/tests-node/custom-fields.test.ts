// Custom fields (decision 0022, rule 8.3): a member of a host's row that the chart has no field for is kept by the row's key and given back in a create's `set` and a delete's `was`. No engine is needed.
import { test } from "node:test";
import { expect } from "@playwright/test";
import { fromTasks, type PlainTask } from "../src/adapters/plain.ts";
import type { Change } from "../src/events.ts";
import { readFields } from "../src/field-map.ts";
import type { Fields } from "../src/gantt-types.ts";

const row = (extra: object = {}) => ({ id: "t1", key: "t1", name: "Pour", start: "2026-03-02", duration: 2, ...extra });
const names = (fields?: Fields) => readFields(fields, () => undefined);
const deleted = (key: string | null, was: object): Change => ({ record: "task", op: "delete", key, task: 1, was: { ...was } });
const created = (key: string | null, set: object): Change => ({ record: "task", op: "create", key, task: 1, set: { ...set } });

test("a member the chart has no field for comes back unchanged in a delete's was and in the create that undoes it", () => {
  const map = names();
  const taken = map.custom.take("tasks", [row({ status: "late", owner: "sam" })]);
  taken.commit();
  expect(taken.rows).toEqual([row()]);
  expect(map.custom.add([deleted("t1", { name: "Pour" })])).toEqual([deleted("t1", { name: "Pour", status: "late", owner: "sam" })]);
  expect(map.custom.add([created("t1", { name: "Pour" })])).toEqual([created("t1", { name: "Pour", status: "late", owner: "sam" })]);
});

test("a row the person made has none, and an update never carries one", () => {
  const map = names();
  map.custom.take("tasks", [row({ owner: "sam" })]).commit();
  const made = created("new1", { name: "New" });
  const edit: Change = { record: "task", op: "update", key: "t1", task: 1, set: { name: "B" }, was: { name: "A" } };
  expect(map.custom.add([made, edit, deleted(null, { name: "x" })])).toEqual([made, edit, deleted(null, { name: "x" })]);
});

test("drop forgets the members of the rows it names and no others", () => {
  const map = names();
  map.custom.take("tasks", [row({ owner: "sam" }), row({ id: "t2", key: "t2", owner: "kim" })]).commit();
  map.custom.drop("tasks", ["t1"]);
  expect(map.custom.add([deleted("t1", { name: "Pour" })]), "the dropped row's members are gone").toEqual([deleted("t1", { name: "Pour" })]);
  expect(map.custom.add([deleted("t2", { name: "Pour" })]), "another row's stay").toEqual([deleted("t2", { name: "Pour", owner: "kim" })]);
});

test("a misspelled member is kept and listed, and fromTasks counts nothing in leftOut", () => {
  const map = names();
  const taken = map.custom.take("tasks", [row({ durration: 3 }), row({ id: "t2", key: "t2" })]);
  taken.commit();
  expect(map.custom.names()).toEqual({ tasks: ["durration"], links: [] });
  const { tasks: listed, links: none } = map.custom.names();
  expect([Object.isFrozen(map.custom.names()), Object.isFrozen(listed), Object.isFrozen(none)], "the names and both lists are frozen").toEqual([true, true, true]);
  expect(fromTasks({ tasks: taken.rows as PlainTask[] }).leftOut).toEqual([]);
  expect(map.custom.add([deleted("t1", {})])[0]!.was).toEqual({ durration: 3 });
});

test("with name mapped to title, a row's own name member is a custom field", () => {
  const map = names({ tasks: { name: "title", key: "id" } });
  const taken = map.custom.take("tasks", [{ id: "b", title: "Pour", start: "2026-03-02", name: "mine" }]);
  taken.commit();
  expect(taken.rows).toEqual([{ id: "b", title: "Pour", start: "2026-03-02" }]);
  expect(map.custom.names().tasks).toEqual(["name"]);
  expect(map.custom.add(map.changes([deleted("b", { name: "Pour" })]))).toEqual([deleted("b", { title: "Pour", name: "mine" })]);
});

test("a link keeps its custom members by its own key, and its names are listed apart from the tasks'", () => {
  const map = names();
  map.custom.take("links", [{ from: "a", to: "b", key: "l1", note: "x" }]).commit();
  expect(map.custom.names()).toEqual({ tasks: [], links: ["note"] });
  const link: Change = { record: "link", op: "delete", key: "l1", link: 0, was: { from: "a", to: "b" } };
  expect(map.custom.add([link])[0]!.was).toEqual({ note: "x", from: "a", to: "b" });
});

test("an update record's custom members are kept only when the chart took the update, and add to what the row had", () => {
  const map = names();
  map.custom.take("tasks", [row({ owner: "sam" })]).commit();
  const refused = map.custom.take("tasks", [{ key: "t1", owner: "kim" }]);
  expect(refused.rows).toEqual([{ key: "t1" }]);
  expect(map.custom.add([deleted("t1", {})])[0]!.was).toEqual({ owner: "sam" });
  refused.commit();
  map.custom.take("tasks", [{ key: "t1", team: "a" }]).commit();
  expect(map.custom.add([deleted("t1", {})])[0]!.was).toEqual({ owner: "kim", team: "a" });
  const dates = [{ key: "t1", hostStart: "2026-03-02", hostFinish: "2026-03-04" }];
  expect(map.custom.take("tasks", dates).rows, "the two host dates only update takes are chart members").toBe(dates);
});

test("a row with no text key keeps its unknown members for fromTasks to count, and clear forgets rows and names", () => {
  const map = names();
  const rows = [{ id: "n", name: "No key", start: "2026-03-02", owner: "sam" }];
  expect(map.custom.take("tasks", rows).rows).toBe(rows);
  map.custom.take("tasks", [row({ owner: "sam" })]).commit();
  const before = map.custom.names();
  expect(map.custom.names()).toBe(before);
  map.custom.clear();
  expect(map.custom.names()).toEqual({ tasks: [], links: [] });
  expect([Object.isFrozen(map.custom.names().tasks), Object.isFrozen(map.custom.names().links)], "the lists after a clear, and at the start, are frozen").toEqual([true, true]);
  expect(map.custom.add([deleted("t1", { name: "Pour" })])[0]!.was).toEqual({ name: "Pour" });
});
