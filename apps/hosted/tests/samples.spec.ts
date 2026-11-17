import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { SAMPLES_DIR, readSamplesJson } from "./samples-file";
import { serveGzipAsStored } from "./support/page";

// The sample menu and `?sample=` (gs#511 S6a, S6b). The expected row counts come from `samples.json`, which
// `scripts/build-samples.mjs` wrote from the generated files themselves, so the spec compares what the page shows
// with what the build counted, and a menu entry nobody checked cannot pass.

interface Entry {
  id: string;
  label: string;
  file: string | null;
  activities: number;
  rows: number;
  bytes: number;
}

const DIR = SAMPLES_DIR;
const SAMPLES: Entry[] = readSamplesJson<{ samples: Entry[] }>().samples;
const COUNT = new Intl.NumberFormat("en-US");

/** The one-line description each menu row shows. */
const DESCRIPTIONS: Record<string, string> = {
  "office-fitout": "A small made-up office fit-out project: 9 phases with 50 activities under them",
  "medium-5k": "A generated XER file of 5,000 activities",
  "large-50k": "A generated XER file of 50,000 activities",
  "demo-50k": "A generated schedule of 50,000 tasks; it opens by reloading the page",
};

function entry(id: string): Entry {
  const found = SAMPLES.find((sample) => sample.id === id);
  if (found === undefined) throw new Error(`samples.json has no ${id}`);
  return found;
}

/** The text the chart's `#row-count` shows when `rows` rows are open. */
function rowsText(rows: number): string {
  return `${COUNT.format(rows)} rows`;
}

/** Wait until the chart is up and has drawn, then return its schedule's task count. */
async function ready(page: Page): Promise<number> {
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, {
    timeout: 180_000,
  });
  return page.evaluate(() => window.gantt!.status().n);
}

/**
 * Wait until the chart either shows `sample`'s rows or reports that it could not read the file, then require the
 * first. A file the importer rejects fails here with the importer's own error line.
 */
async function expectOpened(page: Page, sample: Entry): Promise<void> {
  const rows = page.locator("#row-count", { hasText: rowsText(sample.rows) });
  const failure = page.locator("#open-error-text:visible");
  await expect(rows.or(failure).first()).toBeVisible({ timeout: 120_000 });
  expect(await failure.allTextContents(), "the importer reported an error").toEqual([]);
  await expect(page.locator("#row-count")).toHaveText(rowsText(sample.rows));
}

/**
 * Record every request the page makes, as "METHOD url" with the origin cut off, and a sample's path cut down to
 * `/samples/<file>` so the specs read the same when the page is served under a path prefix (the hosted site serves it at the root).
 */
function recordRequests(page: Page): string[] {
  const requests: string[] = [];
  page.on("request", (request) =>
    requests.push(`${request.method()} ${new URL(request.url()).pathname.replace(/^.*(?=\/samples\/)/, "")}`),
  );
  return requests;
}

function recordErrors(page: Page): string[] {
  const logs: string[] = [];
  page.on("console", (message) => logs.push(`${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => logs.push(`pageerror: ${error.message}`));
  return logs;
}

const errorsOf = (logs: string[]): string[] => logs.filter((line) => /^(error|pageerror)/.test(line));

test("the menu lists \"Open a file…\" and then the four samples with their activity counts and download sizes", async ({ page }) => {
  await page.goto("/");
  await ready(page);
  await page.click("#open-button");
  // The menu is the chart's "Open…" button's (gs#858): "New schedule" comes first, then "Open a file…" and the four samples.
  await expect(page.locator("#sample-list button")).toHaveCount(8); // New schedule, Open a file…, Compare with a file..., the four samples, then Save
  await expect(page.locator("#sample-list button").nth(1)).toContainText("Open a file…");
  for (const id of ["office-fitout", "medium-5k", "large-50k", "demo-50k"]) {
    const sample = entry(id);
    const item = page.locator(`#sample-list button[data-sample-id="${id}"]`);
    await expect(item).toContainText(sample.label);
    // The menu shows the activity count (the rows count is the page's, below).
    await expect(item).toContainText(`${COUNT.format(sample.activities)} activities`);
    await expect(item).toContainText(sample.file === null ? "built in" : "download");
    // Each row says what the sample is for, in the words fixed here (not read back from samples.json, which would pass on any text).
    await expect(item.locator(".sample-meta").first()).toHaveText(DESCRIPTIONS[id]!);
  }
});

for (const id of ["office-fitout", "medium-5k", "large-50k"]) {
  test(`the menu opens ${id} from a gzip file and #row-count reads its rows from samples.json`, async ({ page }) => {
    const logs = recordErrors(page);
    await serveGzipAsStored(page);
    await page.goto("/");
    await ready(page);
    const sample = entry(id);
    const requests = recordRequests(page);
    await page.click("#open-button");
    await page.click(`#sample-list button[data-sample-id="${id}"]`);
    await expectOpened(page, sample);
    // The chart's own task count agrees: the rows are the activities plus the summary rows.
    expect(await page.evaluate(() => window.gantt!.status().n)).toBe(sample.rows);
    expect(sample.rows).toBeGreaterThan(sample.activities);
    // The only request is the sample's file; the open itself reads it in the browser.
    expect(requests).toEqual([`GET /samples/${sample.file}`]);
    await expect(page.locator("#sample-status")).toHaveText("");
    expect(errorsOf(logs), "the page logs no error").toEqual([]);
  });
}

test("choosing a file sample from the menu puts ?sample=<id> in the address bar, and a reload opens it", async ({ page }) => {
  await page.goto("/");
  await ready(page);
  const sample = entry("medium-5k");
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="medium-5k"]`);
  await expectOpened(page, sample);
  expect(new URL(page.url()).searchParams.get("sample")).toBe("medium-5k");
  await page.reload();
  await ready(page);
  await expectOpened(page, sample);
});

test("choosing a sample adds no history entry: Back leaves the page", async ({ page }) => {
  await page.goto("about:blank");
  await page.goto("/");
  await ready(page);
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="medium-5k"]`);
  await expectOpened(page, entry("medium-5k"));
  expect(new URL(page.url()).searchParams.get("sample")).toBe("medium-5k");
  await page.goBack();
  expect(page.url(), "one Back press leaves the page, so the choice added no entry").toBe("about:blank");
});

test("a sample that fails to open leaves the address on the sample that did open", async ({ page }) => {
  await page.route("**/samples/large-50k.xer.gz", (route) => route.fulfill({ status: 404, body: "gone" }));
  await page.goto("/");
  await ready(page);
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="medium-5k"]`);
  await expectOpened(page, entry("medium-5k"));
  const opened = page.url();
  expect(new URL(opened).searchParams.get("sample")).toBe("medium-5k");
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="large-50k"]`);
  await expect(page.locator("#sample-status")).toContainText("large-50k did not open");
  expect(page.url(), "the failed open did not rewrite the address").toBe(opened);
});

test("while large-50k downloads, #sample-status names it by its label and size, not its id", async ({ page }) => {
  await serveGzipAsStored(page);
  await page.goto("/");
  await ready(page);
  const sample = entry("large-50k");
  // Hold the file's response until the status has been read, so the page is mid-download.
  await page.unroute("**/samples/*.gz");
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route("**/samples/large-50k.xer.gz", async (route) => {
    await held;
    await route.fulfill({
      status: 200,
      body: readFileSync(new URL(sample.file!, DIR)),
      headers: { "content-type": "application/gzip" },
    });
  });
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="large-50k"]`);
  const status = page.locator("#sample-status");
  await expect(status).toContainText("Fetching");
  const size = `${(sample.bytes / 1_000_000).toFixed(1)} MB`;
  await expect(status).toHaveText(`Fetching ${sample.label}, ${size}…`);
  await expect(status).toContainText("MB");
  expect(await status.textContent()).not.toContain(sample.id);
  release();
  await expectOpened(page, sample);
  await expect(status).toHaveText("");
});

test("the menu opens demo-50k, the built-in schedule, and #row-count reads its rows", async ({ page }) => {
  const logs = recordErrors(page);
  await page.goto("/");
  await ready(page);
  const sample = entry("demo-50k");
  expect(sample.file).toBeNull();
  const requests = recordRequests(page);
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="demo-50k"]`);
  await page.waitForURL(/[?&]sample=demo-50k/);
  expect(await ready(page)).toBe(sample.rows);
  await expect(page.locator("#row-count")).toHaveText(rowsText(sample.rows));
  expect(requests.filter((line) => line.includes("/samples/") && line.includes(".gz")), "no sample file is fetched").toEqual([]);
  expect(errorsOf(logs), "the page logs no error").toEqual([]);
});

test("?sample=medium-5k opens it straight away, from a server that already undid the gzip", async ({ page }) => {
  const requests = recordRequests(page);
  await page.goto("/?sample=medium-5k");
  await ready(page);
  await expectOpened(page, entry("medium-5k"));
  expect(requests).toContain("GET /samples/medium-5k.xer.gz");
});

test("?sample=nope shows its reason and opens nothing", async ({ page }) => {
  const logs = recordErrors(page);
  const requests = recordRequests(page);
  await page.goto("/?sample=nope");
  const opened = await ready(page);
  const status = page.locator("#sample-status");
  await expect(status).toContainText('There is no sample "nope"');
  await expect(status).toContainText("nothing was opened");
  // The page stays on the small schedule it starts with: no sample was chosen, no file was fetched.
  expect(SAMPLES.map((sample) => sample.rows)).not.toContain(opened);
  expect(requests.filter((line) => line.includes(".gz"))).toEqual([]);
  expect(errorsOf(logs), "the page logs no error").toEqual([]);
});

/** The `table_rows` columns this spec reads: the critical flag (4) and the kind (5: a leaf is 0, a summary 1 or 2). */
type TableRow = [number, string, string, number | null, boolean, 0 | 1 | 2];

test("office-fitout opens with its 50 activities, a critical path and its started activities' actuals applied", async ({ page }) => {
  const logs = recordErrors(page);
  await page.goto("/?sample=office-fitout");
  await ready(page);
  const sample = entry("office-fitout");
  await expectOpened(page, sample);

  // The leaf rows (no children) are counted from the page's own table, against a fixed 50: the table in
  // `samples/office-fitout.ts` cannot move this number, so a leaf deleted from it turns this red (49 leaves). The
  // row count above cannot, because `samples.json` is rebuilt from the same table.
  const rows = await page.evaluate(() => JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as TableRow[]);
  const leaves = rows.filter((row) => row[5] === 0);
  expect(leaves.length, "leaf activities on the page").toBe(50);
  expect(rows.length - leaves.length, "summary tasks on the page").toBe(9);
  expect(sample.activities, "samples.json counts the same activities").toBe(leaves.length);
  expect(sample.rows, "samples.json counts the same rows").toBe(rows.length);

  // A critical path exists, and it is a path through the project, not every activity.
  const critical = leaves.filter((row) => row[4]).length;
  expect(critical, "critical activities").toBeGreaterThan(0);
  expect(critical, "critical activities").toBeLessThan(50);

  // The MS Project reader applies the actual dates and remaining times the sample states for its 13 started activities
  // (101 to 106, 201 to 203, 301 to 304), as the P6 reader does (gs#860): the table has its progress columns, those
  // activities have an actual start, and the panel lists no "actuals ignored" line and no summary-task, link, calendar
  // or constraint line, so the toolbar has no notice.
  expect(await page.evaluate(() => window.gantt!.app.has_progress()), "the table has its progress columns").toBe(true);
  const started = await page.evaluate(() => {
    const count = window.gantt!.status().rowCount;
    let withStart = 0;
    for (let i = 0; i < count; i++) if (window.gantt!.app.task_actual_start(i) !== "") withStart++;
    return withStart;
  });
  expect(started, "tasks with an actual start").toBe(13);
  // The one line left is the reader's note about the crews' assignments (`sample-notes.spec.ts` pins it).
  expect((await page.evaluate(() => window.gantt!.unsupported!)).filter((line) => !/^\d+ tasks have resource assignments/.test(line))).toEqual([]);
  await expect(page.locator("#import-notes"), "the toolbar has no notice").toBeHidden();
  expect(errorsOf(logs), "the page logs no error").toEqual([]);
});
