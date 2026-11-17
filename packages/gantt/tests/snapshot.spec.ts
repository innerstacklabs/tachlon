// `Gantt.snapshot` (gantt#2077): the plain schedule carries the start the chart has solved, not the task's own start.
import { fromTasks } from "../src/adapters/plain";
import { expect, test } from "./support/gantt";

const tasks = [
  { id: "dig", name: "Dig", start: "2026-03-02", duration: 3 },
  { id: "pour", name: "Pour", start: "2026-03-02", duration: 2 },
  { id: "cure", name: "Cure", start: "2026-03-02", duration: 2 },
];
const links = [{ from: "dig", to: "pour" }];
const snapshot = (g: { page: import("@playwright/test").Page }) => g.page.evaluate(() => window.gantt!.api!.snapshot());

test("snapshot_gives_the_start_the_chart_shows", async ({ g }) => {
  await g.open({ n: 20 });
  await g.page.evaluate((document) => window.gantt!.api!.load({ document }), fromTasks({ tasks, links }).document);
  const { tasks: plain, links: back } = await snapshot(g);
  expect(plain.map((t) => [t.id, t.start])).toEqual([["dig", "2026-03-02T08:00"], ["pour", "2026-03-05T08:00"], ["cure", "2026-03-02T08:00"]]);
  expect(back).toEqual([{ from: "dig", to: "pour", type: "FS", lag: 0 }]);
});

test("snapshot_follows_an_inserted_and_a_deleted_row", async ({ g }) => {
  await g.open({ n: 20 });
  await g.page.evaluate((document) => window.gantt!.api!.load({ document }), fromTasks({ tasks, links }).document);
  await g.page.evaluate(() => [window.gantt!.api!.commands.select(0), window.gantt!.api!.commands.insertRow()]);
  const added = (await snapshot(g)).tasks.map((t) => [t.name, t.start]);
  expect(added).toEqual([["Dig", "2026-03-02T08:00"], ["New task", expect.any(String)], ["Pour", "2026-03-05T08:00"], ["Cure", "2026-03-02T08:00"]]);
  await g.page.evaluate(() => [window.gantt!.api!.commands.select(0), window.gantt!.api!.commands.deleteRow()]);
  expect((await snapshot(g)).tasks.map((t) => [t.name, t.start])).toEqual([["New task", added[1]![1]], ["Pour", "2026-03-02T08:00"], ["Cure", "2026-03-02T08:00"]]);
});
