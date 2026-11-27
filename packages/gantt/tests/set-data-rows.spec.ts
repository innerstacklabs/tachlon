import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// gantt#2087: `setData` adds the tasks its rows hold and the chart does not, in place: the selection, the scroll and the undo list stay, and the add is not an edit.
declare global {
  interface Window { upChart: Gantt; edits: unknown[]; mountUp(options: Record<string, unknown>): Promise<void> }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.edits = [];
  window.mountUp = async (options) => {
    window.upChart?.destroy();
    window.upChart = await mount(document.getElementById("app"), { forceWebgl: true, ...options });
    window.upChart.on("edit", (change) => window.edits.push(change));
  };
  window.ready = true;
</script></body></html>`;
let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0, strictPort: false } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(async () => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

const tasks = Array.from({ length: 20 }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2 }));
const added = [
  { id: "n1", key: "n1", name: "Added one", start: "2026-03-09", duration: 3, parent: "t4" },
  { id: "n2", key: "n2", name: "Added two", start: "2026-03-10", duration: 1 },
  { id: "n3", key: "n3", name: "Added three", start: "2026-03-11", duration: 1 },
];
/** The rows with the three added ones: `n1` under `t4`, `n3` at the top level after `t4` (a sibling that has a child), `n2` at the top level after `t9`. */
const rows = (second: object = {}) => [...tasks.slice(0, 5), added[2]!, ...tasks.slice(5, 10), { ...added[1], ...second }, ...tasks.slice(10), added[0]!];

const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
const saved = (page: Page) => page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; name: string; duration: number; parent: number | null }[] }).tasks);
const table = (page: Page) => page.evaluate(() => ({ rows: window.gantt!.status().rowCount, height: document.getElementById("table-body")!.scrollHeight, top: document.getElementById("table-body")!.scrollTop, selected: window.upChart.state().selection?.task }));

const without = (...keys: string[]) => tasks.filter((task) => !keys.includes(task.key));
const owned = tasks.map((task) => (task.key === "k5" || task.key === "k6" ? { ...task, owner: "sam" } : task));
const names = (page: Page) => saved(page).then((now) => now.map((task) => task.key));
const pick = async (page: Page, row: number) => { // select a row of the table as a person does, with the table at the top and no edit recorded yet
  await page.evaluate(() => { document.getElementById("table-body")!.scrollTop = 0; window.edits.length = 0; });
  await page.locator(`#table-rows .row[data-row="${row}"] .name`).click();
};

async function load(page: Page): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
}

async function open(page: Page, mounted: object[] = tasks, links: object[] = []): Promise<void> {
  await load(page);
  await page.evaluate((o) => window.mountUp(o), { tasks: mounted, links });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await page.evaluate(() => { window.edits.length = 0; document.getElementById("table-body")!.scrollTop = 1000; });
  await frames(page);
  await page.locator("#table-rows .row").nth(8).click({ position: { x: 4, y: 4 } });
  await frames(page);
}

test("setData adds the rows its data holds under their parents and after their previous sibling: selection, scroll and the earlier rename's undo hold, the table follows, and it is no edit", async ({ page }) => {
  await open(page);
  const before = await table(page);
  expect(before.top, "the table is scrolled").toBeGreaterThan(0);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows())).toEqual({ ok: true, updated: 3 });
  await frames(page);
  const after = await table(page);
  expect([after.rows, after.height, after.top, after.selected], "the table has the three rows more, and the scroll and the selection are as they were").toEqual([before.rows + 3, before.height + 72, before.top, before.selected]);
  const now = await saved(page);
  expect(now.map((task) => task.key).filter((key) => key === "n1" || key === "n2" || key === "n3" || key === "k4" || key === "k5" || key === "k9" || key === "k10"), "n1 first under k4, n3 after k4 and its child, n2 after k9").toEqual(["k4", "n1", "n3", "k5", "k9", "n2", "k10"]);
  expect(now.find((task) => task.key === "n1"), "the values and the parent as given").toMatchObject({ name: "Added one", duration: 1440, parent: now.findIndex((task) => task.key === "k4") });
  expect(now.find((task) => task.key === "n2"), "the values as given, at the top level").toMatchObject({ name: "Added two", duration: 480, parent: null });
  expect(await page.evaluate(() => window.edits), "an add is not an edit").toEqual([]);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows()), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  const undone = await saved(page);
  expect([undone.find((task) => task.key === "k2")!.name, undone.length], "the one Undo is the person's rename; the added rows stay").toEqual(["Task 2", 23]);
});

test("an added row takes its color and label in place, and a color or label the chart cannot read is left out as mount leaves it out", async ({ page }) => {
  await open(page);
  const look = { color: "#12ab34", label: "Shown" };
  const given = [...tasks, { ...added[1], ...look }];
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), given)).toEqual({ ok: true, updated: 1 });
  const row = await page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; color?: string; label?: string }[] }).tasks.find((task) => task.key === "n2"));
  expect(row, "the color and the label are in the document").toMatchObject(look);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), [...given, { ...added[2], color: "red", label: 7 as never }])).toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; color?: string; label?: string }[] }).tasks.filter((task) => task.key === "n3").map(({ color, label }) => [color, label])), "n3 is added without the color and the label").toEqual([[undefined, undefined]]);
});

const nested = tasks.map((task) => (task.key === "k6" ? { ...task, parent: "t5" } : task));

test("a row a person deleted, the data dropped and Undo brought back is taken in place when the data gives it again: no start again, and the selection, the scroll and the undo list hold", async ({ page }) => {
  await open(page, owned);
  await pick(page, 5);
  await page.evaluate(() => window.upChart.commands.deleteRow());
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k5")), "the drop of a deleted row counts nothing").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  await frames(page);
  const before = { ...(await table(page)), edits: await page.evaluate(() => window.edits.length) };
  const back = owned.map((task) => (task.key === "k5" ? { ...task, name: "Back" } : task));
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), back), "k5 is held again: its name is set, nothing is added").toEqual({ ok: true, updated: 1 });
  await frames(page);
  expect({ ...(await table(page)), edits: await page.evaluate(() => window.edits.length) }, "no row more, the scroll and the selection as they were, and no edit").toEqual(before);
  expect((await saved(page)).filter((task) => task.key === "k5").map((task) => task.name), "one k5, renamed").toEqual(["Back"]);
  expect(await page.evaluate(() => window.upChart.commands.undo()), "the undo list is the person's: the rename that came first").toBe(true);
  expect((await saved(page)).find((task) => task.key === "k2")!.name).toBe("Task 2");
});

test("a child and its parent a person deleted, the data dropped together and Undo brings back one at a time, are each taken in place as they return", async ({ page }) => {
  await open(page, nested);
  for (const row of [6, 5]) { // a person deletes a leaf: the child first, then the parent that is a leaf by then
    await pick(page, row);
    await page.evaluate(() => window.upChart.commands.deleteRow());
  }
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k5", "k6")), "both are skipped").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  const parent = nested.filter((task) => task.key !== "k6").map((task) => (task.key === "k5" ? { ...task, name: "Parent" } : task));
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), parent), "k5 is back, k6 still deleted: only k5's name is set").toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  const both = nested.map((task) => (task.key === "k5" ? { ...task, name: "Parent" } : task.key === "k6" ? { ...task, name: "Child" } : task));
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), both), "k6 is back under k5: only its name is set").toEqual({ ok: true, updated: 1 });
  expect((await saved(page)).filter((task) => task.key === "k5" || task.key === "k6").map((task) => [task.key, task.name]), "one of each").toEqual([["k5", "Parent"], ["k6", "Child"]]);
});

test("a deleted row the data gives again is a new row while it is still deleted, and a move once Undo brought it back", async ({ page }) => {
  await open(page, owned);
  await pick(page, 5);
  await page.evaluate(() => window.upChart.commands.deleteRow());
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k5"))).toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), owned), "still deleted: an add").toEqual({ ok: true, updated: 1 });
  expect((await names(page)).filter((key) => key === "k5")).toEqual(["k5"]);
  await open(page, owned);
  await pick(page, 5);
  await page.evaluate(() => window.upChart.commands.deleteRow());
  await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k5"));
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  const before = await names(page);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), [...without("k5"), owned[5]!]), "k5 at the end is a move").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("moved") });
  expect(await names(page), "nothing changed").toEqual(before);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), owned.map((task) => (task.key === "k5" ? { ...task, parent: "t4" } : task))), "k5 under k4 is a move").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("another id or parent") });
});

test("a bad record on the second add keeps the first and corrected rows add the rest; a key the chart already holds refuses the whole call", async ({ page }) => {
  await open(page);
  const first = await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows().map((task) => (task.key === "n1" ? { ...task, progress: 150 } : task)));
  expect(first, "the first add refused, nothing set before it: the message says nothing changed").toMatchObject({ ok: false, key: "n1", message: expect.stringMatching(/^Nothing changed: [^]*$/) });
  expect((first as { message: string }).message).not.toContain("in the chart");
  const answer = await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows({ progress: 150 }));
  expect(answer).toMatchObject({ ok: false, reason: "badPercent", key: "n2", message: expect.stringContaining("Partly applied") });
  expect((await saved(page)).map((task) => task.key).filter((key) => key?.startsWith("n")), "the adds before it stay").toEqual(["n1", "n3"]);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows({ progress: 50 })), "the rest is added").toEqual({ ok: true, updated: 1 });
  expect((await saved(page)).map((task) => task.key).filter((key) => key?.startsWith("n"))).toEqual(["n1", "n3", "n2"]);
  await page.evaluate(() => window.gantt!.app.add_task(JSON.stringify({ key: "held", name: "Held" }), null, null, false));
  const next = [...rows({ progress: 50 }), { id: "m1", key: "m1", name: "New" }, { id: "held", key: "held", name: "Held" }];
  expect(await page.evaluate((all) => window.upChart.setData({ tasks: all }), next), "a row the chart holds is not an add").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("Nothing changed") });
  expect((await saved(page)).some((task) => task.key === "m1"), "m1 was not added either").toBe(false);
});

test("setData drops a row its data no longer holds: the person's step that names it is forgotten and the other's stays, the selection and the scroll hold, and it is no edit", async ({ page }) => {
  await open(page);
  expect(await page.evaluate(() => window.upChart.hostEdit([{ key: "k3", name: "Second" }])), "a second step, after the rename of k2").toMatchObject({ ok: true });
  await page.evaluate(() => { window.upChart.setPending(["k2", "k12"]); document.getElementById("table-body")!.scrollTop = 30; });
  await frames(page);
  await page.evaluate(() => { window.edits.length = 0; });
  const before = await table(page);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k2"))).toEqual({ ok: true, updated: 1 });
  await frames(page);
  const after = await table(page);
  expect([after.rows, after.height, after.top, after.selected], "a row less, the scroll and the selection as they were").toEqual([before.rows - 1, before.height - 24, before.top, before.selected]);
  expect([(await names(page)).includes("k2"), await page.evaluate(() => window.edits), await page.evaluate(() => [...document.querySelectorAll("#table-rows .row[data-pending]")].map((row) => row.querySelector(".name")?.textContent))], "k2 is gone, with no edit, and only the other mark is left").toEqual([false, [], ["Task 12"]]);
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  expect((await saved(page)).find((task) => task.key === "k3")!.name, "the one Undo is the other row's step").toBe("Task 3");
  expect(await page.evaluate(() => window.upChart.commands.undo()), "the dropped row's step is gone").toBe(false);
});

test("a row a person deleted and the data then drops keeps its Undo and its columns", async ({ page }) => {
  await open(page, owned);
  await pick(page, 5);
  await page.evaluate(() => window.upChart.commands.deleteRow());
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k5")), "the row is not the chart's to remove: it counts nothing").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  await frames(page);
  expect(await names(page), "Undo brings it back").toContain("k5");
  await expect.poll(() => page.evaluate(() => window.edits.at(-1)), "with its own columns").toMatchObject({ commits: [{ changes: [{ op: "create", key: "k5", set: { owner: "sam" } }] }] });
});

test("a dropped row's columns are forgotten: the row given again has none to give back", async ({ page }) => {
  await open(page, owned);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), without("k6"))).toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), tasks), "k6 comes back with no columns").toEqual({ ok: true, updated: 1 });
  await pick(page, 6);
  await page.evaluate(() => window.upChart.commands.deleteRow());
  await frames(page);
  await expect.poll(() => page.evaluate(() => window.edits.at(-1))).toMatchObject({ commits: [{ changes: [{ op: "delete", key: "k6" }] }] });
  await expect.poll(() => page.evaluate(() => JSON.stringify(window.edits.at(-1))), "no owner").not.toContain("owner");
});

test("a summary whose children are all dropped is still a summary while the records are set; a row a person gave a child refuses its drop, and the drops before it stay", async ({ page }) => {
  await open(page, tasks.map((task) => (task.key === "k4" ? { ...task, parent: "t3" } : task)));
  const gone = tasks.filter((task) => task.key !== "k4").map((task) => (task.key === "k3" ? { ...task, duration: 5 } : task));
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), gone), "k3 is a summary until k4 is gone: its duration is refused, and nothing is dropped").toMatchObject({ ok: false, key: "k3", message: expect.stringMatching(/^Nothing changed/) });
  expect(await names(page)).toContain("k4");
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), gone.map((task) => (task.key === "k3" ? tasks[3]! : task))), "the drop alone").toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), gone), "the duration, one call later").toEqual({ ok: true, updated: 1 });
  await page.evaluate(() => window.gantt!.app.add_task(JSON.stringify({ key: "c1", name: "Child" }), "k8", null, true));
  const last = gone.filter((task) => task.key !== "k7" && task.key !== "k8");
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), last), "k7 went, k8 has a child the data does not hold").toMatchObject({ ok: false, reason: "hasChildren", key: "k8", message: expect.stringContaining("Partly applied") });
  expect((await names(page)).filter((key) => key === "k7" || key === "k8" || key === "c1")).toEqual(["k8", "c1"]);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), last), "the same rows again: nothing is applied").toMatchObject({ ok: false, reason: "hasChildren", message: expect.stringMatching(/^Nothing changed/) });
});

/** The links the chart holds as `[pred's key, succ's key, link key, kind, lag in minutes]`. */
const linksOf = (page: Page) => page.evaluate(() => {
  const doc = window.upChart.toDocument() as { tasks: { key?: string }[]; links: { pred: number; succ: number; kind: string; lag: number; key?: string }[] };
  return doc.links.map((link) => [doc.tasks[link.pred]!.key, doc.tasks[link.succ]!.key, link.key ?? null, link.kind, link.lag]);
});

test("setData adds the link its data holds and drops the one it lacks, by key else by its two ends: selection, scroll and the earlier rename's undo hold, and it is no edit", async ({ page }) => {
  await open(page, tasks, [{ from: "t1", to: "t2", key: "l1" }, { from: "t3", to: "t4" }]);
  const before = await table(page);
  expect(before.top, "the table is scrolled").toBeGreaterThan(0);
  const next = [{ from: "t3", to: "t4" }, { from: "t5", to: "t6", key: "l2", type: "SS" as const, lag: 1 }];
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next)).toEqual({ ok: true, updated: 2 });
  await frames(page);
  expect(await table(page), "the scroll and the selection are as they were").toEqual(before);
  expect(await linksOf(page), "l1 is dropped by its key, k5 to k6 is added with its type and a day of lag").toEqual([["k3", "k4", null, "FS", 0], ["k5", "k6", "l2", "SS", 480]]);
  expect(await page.evaluate(() => window.edits), "no edit").toEqual([]);
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next.map((link) => ({ ...link }))), "the same links again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo()), "the one Undo is the person's rename").toBe(true);
  expect([(await saved(page)).find((task) => task.key === "k2")!.name, (await linksOf(page)).length]).toEqual(["Task 2", 2]);
  expect(await page.evaluate(() => window.upChart.commands.undo()), "no step is left").toBe(false);
});

test("setData sets a kept link's type and lag in place, by key else by its two ends, as mount counts the lag: selection, scroll and the earlier rename's undo hold, and it is no edit", async ({ page }) => {
  await open(page, tasks, [{ from: "t1", to: "t2", key: "l1" }, { from: "t3", to: "t4", type: "SS" }]);
  const before = await table(page);
  const next = [{ from: "t1", to: "t2", key: "l1", lag: 2 }, { from: "t3", to: "t4" }];
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next)).toEqual({ ok: true, updated: 2 });
  await frames(page);
  expect(await table(page), "the scroll and the selection are as they were").toEqual(before);
  expect(await linksOf(page), "l1 has two days of lag, k3 to k4 is FS again").toEqual([["k1", "k2", "l1", "FS", 960], ["k3", "k4", null, "FS", 0]]);
  expect(await page.evaluate(() => window.edits), "no edit").toEqual([]);
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next.map((link) => ({ ...link }))), "the same links again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo()), "the one Undo is the person's rename").toBe(true);
  expect([(await saved(page)).find((task) => task.key === "k2")!.name, await linksOf(page)]).toEqual(["Task 2", [["k1", "k2", "l1", "FS", 960], ["k3", "k4", null, "FS", 0]]]);
});

test("setData re-points a keyed link in place: the engine holds the new link under the key and none on the old pair, selection, scroll and the earlier rename's undo hold, and it is no edit", async ({ page }) => {
  await open(page, tasks, [{ from: "t1", to: "t2", key: "l1" }, { from: "t3", to: "t4" }]);
  const before = await table(page);
  const next = [{ from: "t1", to: "t5", key: "l1", lag: 1 }, { from: "t3", to: "t4" }];
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next)).toEqual({ ok: true, updated: 2 });
  await frames(page);
  expect(await table(page), "the scroll and the selection are as they were").toEqual(before);
  expect(await linksOf(page), "l1 joins k1 to k5 with a day of lag, and no link is left on k1 to k2").toEqual([["k3", "k4", null, "FS", 0], ["k1", "k5", "l1", "FS", 480]]);
  expect(await page.evaluate(() => window.edits), "no edit").toEqual([]);
  expect(await page.evaluate((links) => window.upChart.setData({ links }), next.map((link) => ({ ...link }))), "the same links again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo()), "the one Undo is the person's rename").toBe(true);
  expect([(await saved(page)).find((task) => task.key === "k2")!.name, await linksOf(page)]).toEqual(["Task 2", [["k3", "k4", null, "FS", 0], ["k1", "k5", "l1", "FS", 480]]]);
  expect(await page.evaluate(() => window.upChart.commands.undo()), "no step is left").toBe(false);
});

test("a link the engine refuses answers with its key and Partly applied, keeps the links before it, and corrected rows add the rest", async ({ page }) => {
  await open(page);
  const loop = [{ from: "t7", to: "t8", key: "a" }, { from: "t8", to: "t7", key: "b" }];
  expect(await page.evaluate((links) => window.upChart.setData({ links }), loop), "the second closes a loop").toMatchObject({ ok: false, key: "b", message: expect.stringContaining("Partly applied") });
  expect(await linksOf(page), "the first stays").toEqual([["k7", "k8", "a", "FS", 0]]);
  expect(await page.evaluate((links) => window.upChart.setData({ links }), [loop[0]!, { from: "t8", to: "t9", key: "b" }])).toEqual({ ok: true, updated: 1 });
  expect(await linksOf(page)).toEqual([["k7", "k8", "a", "FS", 0], ["k8", "k9", "b", "FS", 0]]);
});

test("an added link's lag is counted as mount counts it, in each lagCalendar value", async ({ page }) => {
  await load(page);
  const calendars = ["a", "b", "c"].map((key, i) => ({ key, workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 / 2 ** i }));
  const pair = [{ id: "a", key: "a", name: "A", start: "2026-03-02", duration: 1, calendar: "a" }, { id: "b", key: "b", name: "B", start: "2026-03-03", duration: 1, calendar: "b" }];
  const link = [{ from: "a", to: "b", lag: 2 }];
  const seen: unknown[] = [];
  for (const lagCalendar of ["successor", "predecessor", "24h", { calendar: "c" }]) {
    const options = { calendars, durationUnit: "days", lagCalendar, tasks: pair };
    await page.evaluate((o) => window.mountUp(o), { ...options, links: link });
    const mounted = await linksOf(page);
    await page.evaluate((o) => window.mountUp(o), options);
    expect(await page.evaluate((links) => window.upChart.setData({ links }), link)).toEqual({ ok: true, updated: 1 });
    expect(await linksOf(page), JSON.stringify(lagCalendar)).toEqual(mounted);
    seen.push(mounted[0]![4]);
  }
  expect(seen, "two days of the successor's, the predecessor's, a 24-hour and the named calendar's day").toEqual([480, 960, 2880, 240]);
});
