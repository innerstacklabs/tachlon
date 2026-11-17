import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

// tl#1328: a chart mounted with `tasks` and `links` lets the page's list go once it has started, and a switch to WebGL2 afterwards starts
// again from the schedule the running chart holds. The spec plays a host that keeps the list only through `WeakRef`s. The two checks run
// apart: a collection destroys a WebGPU device.

type Snapshot = { document: string; list: string; rows: number; notes: readonly string[] };
/** `gone` collects garbage and says which of the list's two arrays are gone. */
interface Host { ready?: boolean; mount(forceWebgl: boolean): Promise<void>; snapshot(): Snapshot; gone(): Promise<boolean[]> }
type Hook = { failNextFrame(panic: boolean): void; status(): { backend: string; frames: number } };

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"></head>
<body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  const refs = [];
  const given = () => {
    const tasks = [
      { id: 1, name: "Foundation", start: "2026-11-02" },
      { id: 2, name: "Dig", start: "2026-11-02", duration: 3, parent: 1 },
      { id: 3, name: "Pour", start: "2026-11-02", duration: 2, parent: 1, progress: 40 },
      { id: 4, name: "Frame", start: "2026-11-02", duration: 5 },
      { id: 5, name: "Roof", start: "2026-11-02", duration: 4, crew: "north" },
    ];
    const links = [{ from: 2, to: 3 }, { from: 3, to: 4, type: "SS", lag: 1 }, { from: 4, to: 5 }];
    refs.push(new WeakRef(tasks), new WeakRef(links));
    return { tasks, links };
  };
  let chart;
  const wait = () => new Promise((done) => setTimeout(done, 200));
  window.__host = {
    mount: async (forceWebgl) => { chart = await mount(document.getElementById("app"), { ...given(), forceWebgl }); },
    snapshot: () => ({ document: JSON.stringify(chart.toDocument()), list: window.gantt.app.list_text(), rows: chart.state().rowCount, notes: chart.state().importNotes }),
    gone: async () => { await wait(); window.gc(); await wait(); return refs.map((ref) => ref.deref() === undefined); },
  };
  window.__host.ready = true;
</script></body></html>`;

let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0, strictPort: false } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(async () => {
  await server.close();
});
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu", "--js-flags=--expose-gc"] } });

async function openHost(page: Page): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { __host?: Host }).__host?.ready === true, undefined, { timeout: 120_000 });
}

test("a chart mounted with tasks and links does not keep the page's list once it has started", async ({ page }) => {
  await openHost(page);
  const gone = await page.evaluate(async () => {
    await (window as unknown as { __host: Host }).__host.mount(true);
    return (window as unknown as { __host: Host }).__host.gone();
  });
  expect(gone, "neither the tasks array nor the links array is reachable").toEqual([true, true]);
});

test("the switch to WebGL2 of a chart mounted with tasks and links keeps the same rows, schedule and notes", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await openHost(page);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  const before = await page.evaluate(async () => {
    await (window as unknown as { __host: Host }).__host.mount(false);
    const h = (window as unknown as { gantt: Hook }).gantt;
    const backend = h.status().backend;
    const snapshot = (window as unknown as { __host: Host }).__host.snapshot();
    if (backend === "WebGPU") h.failNextFrame(false);
    return { backend, snapshot };
  });
  expect(before.backend, "the chart mounted on WebGPU, so there is a WebGPU failure to switch from").toBe("WebGPU");
  expect(before.snapshot.rows, "the list is drawn").toBe(5);
  expect(before.snapshot.notes.length, "the unread field is a note, which the restart must keep").toBeGreaterThan(0);
  // While the chart switches, the hook is down and reads as not yet.
  await page.waitForFunction(() => { try { const h = (window as unknown as { gantt: Hook }).gantt; return h.status().backend === "WebGL2" && h.status().frames >= 5; } catch { return false; } }, undefined, { timeout: 60_000 });
  const after = await page.evaluate(() => (window as unknown as { __host: Host }).__host.snapshot());
  expect(after.rows, "the same rows").toBe(before.snapshot.rows);
  expect(after.list, "the same rows and dates").toBe(before.snapshot.list);
  expect(after.document, "the same schedule document").toBe(before.snapshot.document);
  expect(after.notes, "the same import notes").toEqual(before.snapshot.notes);
  expect(pageErrors, "no page error").toEqual([]);
});

test("when toDocument throws, the WebGL2 switch of a chart mounted with tasks fails with the MountError message", async ({ page }) => {
  await openHost(page);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  await page.evaluate(async () => {
    await (window as unknown as { __host: Host }).__host.mount(false);
    const h = (window as unknown as { gantt: Hook & { app: { save(name: string): string } } }).gantt;
    h.app.save = () => { throw new Error("no document"); }; // what `toDocument` reads
    h.failNextFrame(false);
  });
  await expect(page.locator("#app [role=alert]"), "no list is left to start from, so the switch fails with the MountError's sentence").toContainText("WebGL2 could not start: The chart could not read its schedule to start again");
});
