import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// gs#1087: a product mounts a bare chart and the chart brings no toolbar code. Chart A is mounted with no `installChrome`: the toolbar's
// slots stay empty and the controls the app's chrome owns (the rows, level, zoom, baseline and data date controls, the row count) are not
// there, yet the chart draws and its commands work. Chart B has the chrome put in, and holds every one of those controls.
const CHROME_PARTS = ["open-button", "export-button", "checks-button", "open-input", "open-info", "unsaved-mark", "import-notes", "keys-button", "open-error", "picker", "keys-help", "export-dialog", "checks-dialog", "unsupported", "insert-row", "delete-row", "level-1", "level-2", "level-all", "zoom-in", "zoom-out", "baseline-set", "baseline-add", "baseline-clear", "data-date", "data-date-input", "row-count"];

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="a" class="gantt" tabindex="0" style="width: 1200px; height: 300px"></div>
<div id="b" class="gantt" tabindex="0" style="width: 1200px; height: 300px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  import { installChrome } from "/@id/@gantt/chrome";
  const bare = {};
  for (const key of ["a", "b"]) {
    const api = await mount(document.getElementById(key), { schedule: { demo: 30 }, forceWebgl: true });
    if (key === "b") installChrome(api, document.getElementById(key));
    bare[key] = { api, hook: window.gantt };
  }
  window.bare = bare;
</script></body></html>`;

// A bare chart alone on a page with a title of its own, and a count of the `beforeunload` listeners the page is given.
const ALONE = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="a" class="gantt" tabindex="0" style="width: 1200px; height: 300px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  window.alone = await mount(document.getElementById("a"), { schedule: { demo: 30 }, forceWebgl: true });
</script></body></html>`;

declare global {
  interface Window {
    bare: Record<"a" | "b", { api: Gantt; hook: { status(): { frames: number } } }>;
    alone: Gantt;
    unloadListeners: number;
  }
}

let server: ViteDevServer;
let base = "";

test.beforeAll(async () => {
  server = await createServer({
    root: fileURLToPath(new URL("..", import.meta.url)),
    logLevel: "silent",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
  });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});

test.afterAll(async () => {
  await server.close();
});

test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });

async function open(page: Page): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => window.bare?.a !== undefined && window.bare?.b !== undefined, undefined, { timeout: 180_000 });
}

test("a chart mounted without the chrome has none of the toolbar's controls, and still draws and answers its commands", async ({ page }) => {
  await open(page);
  const found = await page.evaluate((names) => names.map((n) => document.querySelectorAll(`#a [data-part="${n}"]`).length), CHROME_PARTS);
  expect(Object.fromEntries(CHROME_PARTS.map((n, i) => [n, found[i]])), "controls a bare chart carries").toEqual(Object.fromEntries(CHROME_PARTS.map((n) => [n, 0])));
  // Progress and Zoom (its Fit button) stay in the strip; no empty File, Baseline, Rows or Level group is left in it.
  expect(await page.locator("#a [data-part=toolbar] .tb-group:visible").count(), "the groups the strip shows").toBe(2);
  const before = await page.evaluate(() => window.bare.a.hook.status().frames);
  const answers = await page.evaluate(() => ({ row: window.bare.a.api.commands.insertRow()?.ok, rows: window.bare.a.api.state().rowCount }));
  expect(answers.row).toBe(true);
  expect(answers.rows).toBeGreaterThan(0);
  await page.waitForFunction((k) => window.bare.a.hook.status().frames > k + 3, before);
});

// gs#1088: the tab title and the unsaved warning are the host's, through `state()` and the events; the chart writes and listens for neither.
test("a chart mounted without the chrome writes no tab title and listens for no unload, and still opens a file", async ({ page }) => {
  await page.addInitScript(() => {
    window.unloadListeners = 0;
    const add = window.addEventListener.bind(window);
    window.addEventListener = ((type: string, ...rest: [EventListenerOrEventListenerObject]) => {
      // The dev server's own client listens too: only a listener added from the page's or the chart's source counts.
      if (type === "beforeunload" && !(new Error().stack ?? "").includes("@vite")) window.unloadListeners += 1;
      return add(type, ...rest);
    }) as typeof window.addEventListener;
  });
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: ALONE }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => window.alone !== undefined, undefined, { timeout: 180_000 });
  const bytes = [...readFileSync(new URL("./fixtures/one-project.xer", import.meta.url))];
  await page.evaluate((b) => window.alone.open(new File([new Uint8Array(b)], "one-project.xer")), bytes);
  await page.waitForFunction(() => window.alone.state().fileName === "one-project.xer");
  await page.evaluate(() => window.alone.commands.insertRow());
  await page.waitForFunction(() => window.alone.state().unsaved);
  expect(await page.title(), "the page keeps its own title").toBe("host");
  expect(await page.evaluate(() => window.unloadListeners), "beforeunload listeners the chart added").toBe(0);
});

test("the chrome puts every one of those controls into the chart it is installed on", async ({ page }) => {
  await open(page);
  for (const name of CHROME_PARTS) await expect(page.locator(`#b [data-part="${name}"]`), name).toHaveCount(1);
});
