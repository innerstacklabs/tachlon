import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Change, Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// decision 0022, rule 8.3: a member of a host's row that the chart has no field for is kept by key and comes back in the create that undoes a delete.
declare global {
  interface Window { cfChart: Gantt; heard: { changes: Change[] }[]; mountCf(options: Record<string, unknown>): Promise<void> }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.heard = [];
  window.mountCf = async (options) => {
    window.cfChart = await mount(document.getElementById("app"), { forceWebgl: true, ...options });
    window.cfChart.on("edit", (edit) => window.heard.push(...edit.commits));
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

const rows = (owner: string) => Array.from({ length: 5 }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2, owner: `${owner}${i}`, ...(i === 2 ? { durration: 3 } : {}) }));
const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
/** Delete row 2 (key k2) and undo it; the one delete and the one create the chart told the host of. */
async function deleteAndUndo(page: Page): Promise<{ was: unknown; set: unknown }> {
  await page.evaluate(() => { window.heard.length = 0; });
  await page.locator('#table-rows .row[data-row="2"]').click({ position: { x: 4, y: 4 } });
  expect(await page.evaluate(() => window.cfChart.commands.deleteRow())).toMatchObject({ ok: true });
  expect(await page.evaluate(() => window.cfChart.commands.undo())).toBe(true);
  await page.waitForFunction(() => window.heard.length === 2);
  const [del, undo] = await page.evaluate(() => window.heard.map((commit) => commit.changes.find((change) => change.record === "task")!));
  return { was: del!.was, set: undo!.set };
}

test("a host's own members are kept: state lists them, a delete and its undo give them back, an update and setData change them, a typo is not left out", async ({ page }) => {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((o) => window.mountCf(o), { tasks: rows("a") });
  await frames(page);
  expect((await page.evaluate(() => window.cfChart.state().customFields)).tasks, "state lists each name once").toEqual(["owner", "durration"]);
  expect((await page.evaluate(() => window.cfChart.state().importNotes)).join(" "), "a misspelled member is kept, not counted").not.toContain("durration");
  expect(await deleteAndUndo(page), "the undo's create restores the columns").toMatchObject({ was: { name: "Task 2", owner: "a2", durration: 3 }, set: { name: "Task 2", owner: "a2", durration: 3 } });
  expect(await page.evaluate(() => window.cfChart.update([{ key: "k2", owner: "kim" } as never]))).toEqual({ ok: true, updated: 1 });
  expect(await deleteAndUndo(page), "an update changed what the host's row holds").toMatchObject({ was: { owner: "kim" }, set: { owner: "kim" } });
  expect(await page.evaluate((r) => window.cfChart.setData({ tasks: r }), rows("a").map((row, i) => (i === 2 ? { ...row, owner: "lee" } : row)))).toMatchObject({ ok: true });
  expect(await deleteAndUndo(page), "so did setData").toMatchObject({ was: { owner: "lee" }, set: { owner: "lee" } });
});
