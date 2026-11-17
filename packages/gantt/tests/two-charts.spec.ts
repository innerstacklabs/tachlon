import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

// Several charts on one page (gs#1083). The spec plays a host page with two roots, A and B, each mounted with a schedule of
// its own: a row click and a zoom in one leave the other as it was, no element id repeats on the page, a frame error in A shows
// in A while B goes on drawing, and a panic, which leaves the one WASM module unusable, stops both and both report it once. A
// mount that rejects first (a file that is not a schedule) gives its chart number back, so A is still the page's first chart. The page
// is served from a Vite dev server, as `destroy.spec.ts` serves its host page. Both charts draw on WebGL2: an automatic WebGPU
// run answers a failed frame by reloading the page, which is a different path (startup.spec.ts).

/** One chart as the host page keeps it: what `mount` returned, the test hook it published, and the `error` events it fired. */
interface Kept {
  api: { on(name: "error", handler: (detail: { message: string }) => void): () => void };
  hook: { status(): { n: number; frames: number; pxPerMinute: number; selected: { task: number } | null }; failNextFrame(panic: boolean): void };
  errors: string[];
}
type Charts = Record<"a" | "b", Kept>;
/** What the host page keeps of the mount that rejects: the error's stage. */
interface Refused {
  refused: string;
}

const SIZES = { a: 300, b: 120 };

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="x" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<div id="a" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<div id="b" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  import { installChrome } from "/@id/@gantt/chrome";
  // A mount that rejects, in a root the host then removes, as a host that retries in a fresh root would.
  const x = document.getElementById("x");
  window.refused = await mount(x, { schedule: new File(["not a schedule"], "bad.xer"), forceWebgl: true }).then(() => "mounted", (err) => err.stage);
  x.remove();
  const charts = {};
  for (const [key, n] of ${JSON.stringify(Object.entries(SIZES))}) {
    const api = await mount(document.getElementById(key), { schedule: { demo: n }, forceWebgl: true });
    // The app's chrome holds the Zoom buttons the spec presses.
    installChrome(api, document.getElementById(key));
    const errors = [];
    api.on("error", (detail) => errors.push((detail.fatal ? "fatal: " : "") + detail.message));
    // The hook \`mount\` published is this chart's until the next mount replaces \`window.gantt\`, so it is kept here.
    charts[key] = { api, hook: window.gantt, errors };
  }
  window.charts = charts;
</script></body></html>`;

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

const status = (page: Page, key: "a" | "b") =>
  page.evaluate((k) => (window as unknown as { charts: Charts }).charts[k].hook.status(), key);

const errors = (page: Page, key: "a" | "b") =>
  page.evaluate((k) => [...(window as unknown as { charts: Charts }).charts[k].errors], key);

/** A part of one chart, by the name its markup gives it. */
const part = (page: Page, key: "a" | "b", name: string): Locator => page.locator(`#${key} [data-part="${name}"]`);

/** B's frame count `ms` from now less its count now: above zero while B's loop draws. */
async function framesOver(page: Page, ms: number): Promise<number> {
  const before = (await status(page, "b")).frames;
  await page.waitForTimeout(ms);
  return (await status(page, "b")).frames - before;
}

test("two charts on one page keep their own ids, selection, zoom and frame errors", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { charts?: Charts }).charts !== undefined, undefined, { timeout: 120_000 });
  await expect.poll(async () => (await status(page, "b")).frames, { timeout: 60_000 }).toBeGreaterThan(5);
  expect((await status(page, "a")).n, "A has its own schedule").toBe(SIZES.a);
  expect((await status(page, "b")).n, "B has its own schedule").toBe(SIZES.b);
  expect(await page.evaluate(() => (window as unknown as Refused).refused), "the first mount rejected").toBe("schedule");
  const canvasIds = await page.evaluate(() => ["a", "b"].map((key) => document.querySelector(`#${key} [data-part=timeline]`)?.id));
  expect(canvasIds, "after the rejected mount, A is the first chart and writes the bare ids; B writes its prefix").toEqual(["timeline", "chart-2-timeline"]);

  // No id repeats, and every reference by id from a chart's element names an element of the same chart.
  const ids = await page.evaluate(() => {
    const seen = new Map<string, number>();
    for (const el of document.querySelectorAll("[id]")) seen.set(el.id, (seen.get(el.id) ?? 0) + 1);
    const repeated = [...seen].filter(([, count]) => count > 1).map(([id]) => id);
    const crossed: string[] = [];
    for (const root of document.querySelectorAll(".gantt")) {
      for (const el of root.querySelectorAll("[aria-labelledby], [aria-describedby], [aria-controls], label[for]")) {
        const refs = ["aria-labelledby", "aria-describedby", "aria-controls", "for"].flatMap((name) => (el.getAttribute(name) ?? "").split(/\s+/));
        for (const ref of refs.filter((r) => r !== "")) {
          if (!root.contains(document.getElementById(ref))) crossed.push(`#${root.id} ${ref}`);
        }
      }
    }
    return { repeated, crossed, count: seen.size };
  });
  expect(ids.repeated, "no id is on the page twice").toEqual([]);
  expect(ids.crossed, "each id reference stays inside its chart").toEqual([]);

  // A row click in A selects in A and leaves B unselected; then a click in B selects in B and A keeps its selection.
  await part(page, "a", "table-rows").locator(".row").nth(2).click();
  await expect.poll(async () => (await status(page, "a")).selected?.task ?? null).not.toBeNull();
  const inA = (await status(page, "a")).selected!.task;
  expect((await status(page, "b")).selected, "B stays unselected").toBeNull();
  await part(page, "b", "table-rows").locator(".row").nth(1).click();
  await expect.poll(async () => (await status(page, "b")).selected?.task ?? null).not.toBeNull();
  expect((await status(page, "a")).selected?.task, "A keeps its selection").toBe(inA);

  // A zoom in A leaves B's scale alone.
  const scaleB = (await status(page, "b")).pxPerMinute;
  const scaleA = (await status(page, "a")).pxPerMinute;
  await page.locator("#a").getByRole("button", { name: "Zoom in" }).click();
  await expect.poll(async () => (await status(page, "a")).pxPerMinute).toBeGreaterThan(scaleA);
  expect((await status(page, "b")).pxPerMinute, "B's scale is as it was").toBe(scaleB);

  // A frame error in A shows A's banner; B's stays hidden and B goes on drawing.
  await page.evaluate(() => (window as unknown as { charts: Charts }).charts.a.hook.failNextFrame(false));
  await expect(part(page, "a", "frame-error")).toBeVisible();
  await expect(part(page, "a", "frame-error-text")).toContainText("deliberate frame error");
  expect(await framesOver(page, 500), "B's frame count still rises").toBeGreaterThan(0);
  await expect(part(page, "b", "frame-error"), "B shows no error").toBeHidden();
  expect(await errors(page, "a")).toEqual([expect.stringContaining("deliberate frame error")]);
  expect(await errors(page, "b"), "B reported nothing").toEqual([]);

  // A panic in A leaves the one module unusable: both charts stop and both report it. A host page gave no restart, so the banner offers
  // no Reload of its own: the `fatal` error is the host's cue to reload (gs#1217).
  await page.evaluate(() => (window as unknown as { charts: Charts }).charts.a.hook.failNextFrame(true));
  for (const key of ["a", "b"] as const) {
    await expect(part(page, key, "frame-error-text"), `${key} shows the panic`).toContainText("deliberate panic in frame");
    await expect(part(page, key, "frame-error-text")).toContainText("The chart stopped");
    await expect(part(page, key, "frame-error-text")).not.toContainText("Reload to recover");
    await expect(part(page, key, "frame-error-reload"), `${key}: the component offers no reload on a host page`).toBeHidden();
  }
  expect(await framesOver(page, 500), "B stopped drawing").toBe(0);
  // Each chart reports the panic exactly once: A after its frame error, B alone.
  const panics = (lines: string[]): number => lines.filter((line) => line.includes("deliberate panic in frame")).length;
  const reportedA = await errors(page, "a");
  expect(reportedA, "A reported its frame error, then the panic").toHaveLength(2);
  expect(panics(reportedA), "A reported the panic once").toBe(1);
  const reportedB = await errors(page, "b");
  expect(reportedB, "B reported the panic and nothing else").toHaveLength(1);
  expect(panics(reportedB), "B reported the panic once").toBe(1);
  expect([reportedA[1], reportedB[0]].every((line) => line?.startsWith("fatal: ")), "the panic is reported as fatal in both charts").toBe(true);
  expect(pageErrors).toEqual([]);
});

// A chart that closes gives its number back and its keys go with it: mount A and B, destroy A, mount C. C takes A's number (1) and
// writes the bare ids; B keeps 2. A key pressed in one chart moves that chart only.
const LIFECYCLE_HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="a" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<div id="b" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<div id="c" class="gantt" tabindex="0" style="width: 1200px; height: 380px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  const open = async (key) => {
    const api = await mount(document.getElementById(key), { schedule: { demo: 60 }, forceWebgl: true });
    return { api, hook: window.gantt };
  };
  window.charts = { a: await open("a"), b: await open("b") };
  window.charts.a.api.destroy();
  window.charts.c = await open("c");
</script></body></html>`;

test("a closed chart's number goes to the next chart and a key stays in the chart it was pressed in", async ({ page }) => {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: LIFECYCLE_HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { charts?: Record<string, unknown> }).charts?.c !== undefined, undefined, { timeout: 120_000 });
  const scale = (key: string) =>
    page.evaluate((k) => (window as unknown as { charts: Record<string, Charts["a"]> }).charts[k]!.hook.status().pxPerMinute, key);
  const canvasIds = await page.evaluate(() => ["c", "b"].map((key) => document.querySelector(`#${key} [data-part=timeline]`)?.id));
  expect(canvasIds, "C took A's number and writes the bare ids; B keeps 2").toEqual(["timeline", "chart-2-timeline"]);
  expect(await page.locator("#a").innerHTML(), "A's root is empty").toBe("");
  const [scaleB, scaleC] = [await scale("b"), await scale("c")];
  await page.locator("#c").focus();
  await page.keyboard.press("+");
  await expect.poll(() => scale("c")).toBeGreaterThan(scaleC);
  expect(await scale("b"), "a key in C leaves B's scale alone").toBe(scaleB);
  const zoomedC = await scale("c");
  await page.locator("#b").focus();
  await page.keyboard.press("+");
  await expect.poll(() => scale("b")).toBeGreaterThan(scaleB);
  expect(await scale("c"), "a key in B leaves C's scale alone").toBe(zoomedC);
});

test("a file dropped on one chart opens there: the guard of the other chart does not turn its drag into a refusal", async ({ page }) => {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { charts?: Charts }).charts !== undefined, undefined, { timeout: 120_000 });
  // A real drag from the browser: it fires `drop` only when the last `dragover` left a drop effect other than none.
  const cdp = await page.context().newCDPSession(page);
  const box = (await page.locator("#a").boundingBox())!;
  const data = { items: [], files: [fileURLToPath(new URL("./fixtures/one-project.xer", import.meta.url))], dragOperationsMask: 1 };
  for (const type of ["dragEnter", "dragOver", "drop"] as const) await cdp.send("Input.dispatchDragEvent", { type, x: box.x + 200, y: box.y + 200, data });
  await expect.poll(async () => (await status(page, "a")).n, { timeout: 60_000 }).toBe(14);
  expect((await status(page, "b")).n, "B keeps its own schedule").toBe(SIZES.b);
});
