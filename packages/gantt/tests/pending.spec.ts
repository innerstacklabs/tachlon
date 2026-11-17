import { expect, test, type Gantt } from "./support/gantt";
import { fromTasks } from "../src/adapters/plain";

// gantt#2087: a host marks the rows it is recalculating, by key; the table dims them and sets aria-busy until the host clears the mark.
const tasks = Array.from({ length: 30 }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2 }));
const doc = fromTasks({ tasks, links: [] }).document;
const marked = (g: Gantt) => g.page.evaluate(() => [...window.document.querySelectorAll("#table-rows .row[data-pending]")].map((r) => [(r as HTMLElement).dataset["task"], r.getAttribute("aria-busy")]));
const scrollTo = (g: Gantt, top: number) => g.page.evaluate((y) => { window.document.querySelector("#table-body")!.scrollTop = y; }, top);

test("setPending marks the rows by key, keeps them selectable, holds through a scroll, and a new schedule clears it", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 320 });
  await g.open({ n: 20, params: { backend: "webgl" } });
  await g.page.evaluate((d) => window.gantt!.api!.load({ document: d }), doc);
  await g.nextFrame(3);
  const [a, b] = await g.page.evaluate(() => JSON.parse(window.gantt!.app.task_slots(["k1", "k2"])) as number[]);
  expect(await g.page.evaluate(() => window.gantt!.api!.setPending(["k1", "nope", "k2"]))).toEqual(["nope"]);
  await g.nextFrame(3);
  expect(await marked(g), "the two known rows and no other").toEqual([[String(a), "true"], [String(b), "true"]]);
  await g.page.locator(`#table-rows .row[data-task="${b}"]`).click({ position: { x: 4, y: 4 } });
  await g.nextFrame(3);
  expect((await g.status()).selected?.task, "a marked row still selects").toBe(b);
  await scrollTo(g, 1e6);
  await g.nextFrame(4);
  expect(await marked(g), "scrolled out of the drawn rows").toEqual([]);
  await scrollTo(g, 0);
  await g.nextFrame(4);
  expect(await marked(g), "the mark came back with its rows").toHaveLength(2);
  await g.page.evaluate(() => window.gantt!.api!.setPending([]));
  await g.nextFrame(3);
  expect(await marked(g), "an empty list clears every mark").toEqual([]);
  await g.page.evaluate(() => window.gantt!.api!.setPending(["k1"]));
  await g.page.evaluate((d) => window.gantt!.api!.load({ document: d }), doc);
  await g.nextFrame(3);
  expect(await g.page.locator("#table-rows .row").count(), "the new schedule is drawn").toBeGreaterThan(0);
  expect(await marked(g), "opening another schedule clears the marks").toEqual([]);
  await g.page.evaluate(async (d) => { await window.gantt!.api!.load({ document: d }); window.gantt!.api!.setPending(["k2"]); }, doc);
  await g.nextFrame(3);
  expect(await marked(g), "a mark made right after a load stays").toHaveLength(1);
});
