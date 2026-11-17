import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { expect, test, type Page } from "@playwright/test";
import { readSamplesJson } from "./samples-file";

// Our own generated P6 samples open without an import-notes panel over the chart (gs#818). The notes the generator
// reports by construction are marked expected in `samples.json`; the one about the file's in-progress dates reports a
// real difference between the file and the engine, so it stays, as a one-line notice in the toolbar.

interface Entry {
  id: string;
  rows: number;
  expectedNotes?: string[];
}

const SAMPLES = readSamplesJson<{ samples: Entry[] }>().samples;
const COUNT = new Intl.NumberFormat("en-US");

/** The line the reader adds for in-progress activities whose dates in the file differ from the engine's. */
const DATES_LINE = /^in-progress activities: \d+ of \d+ show different dates in the file than the engine computed/;

/** The counted line the reader adds for tasks with an actual date after the data date. */
const ACTUALS_LINE = /^tasks with an actual date after the data date, kept as the file states them: \d+ \(first: /;

async function openSample(page: Page, id: string): Promise<Entry> {
  const sample = SAMPLES.find((candidate) => candidate.id === id)!;
  await page.goto("/");
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, { timeout: 180_000 });
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="${id}"]`);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(sample.rows)} rows`, { timeout: 180_000 });
  await page.waitForFunction(() => window.gantt!.status().frames >= 10);
  return sample;
}

for (const id of ["medium-5k", "large-50k"]) {
  test(`${id} opens with no panel over the chart, and one notice for the real notes`, async ({ page }) => {
    const sample = await openSample(page, id);
    // Nothing covers the chart or the toolbar.
    await expect(page.locator("#unsupported")).toBeHidden();
    const covered = await page.evaluate(() => {
      const box = (selector: string): DOMRect => document.querySelector(selector)!.getBoundingClientRect();
      const at = (x: number, y: number): Element | null => document.elementFromPoint(x, y);
      const canvas = box("#timeline");
      const insert = box("#insert-row");
      const toolbar = document.querySelector("#toolbar")!;
      return {
        corner: at(canvas.right - 8, canvas.top + 8)?.id,
        insert: at(insert.left + insert.width / 2, insert.top + insert.height / 2)?.id,
        notice: toolbar.contains(document.querySelector("#import-notes")),
      };
    });
    expect(covered.corner, "the chart's top right corner is the chart").toBe("timeline");
    expect(covered.insert, "Insert row is not covered").toBe("insert-row");
    expect(covered.notice, "the notice, if any, is in the toolbar").toBe(true);

    // The reader still reports every line; only the expected ones are kept out of the notice. The dates line is not
    // expected, nor is the actuals note (the generated files have tasks with an actual after their data date, and the
    // reader counts them in one note, gantt#2073), so those two are the notes left, behind a one-line notice.
    const all = await page.evaluate(() => window.gantt!.unsupported!);
    expect(all.length, "the reader reports six lines for a generated file").toBe(6);
    expect(all.filter((line) => DATES_LINE.test(line))).toHaveLength(1);
    expect(all.filter((line) => ACTUALS_LINE.test(line))).toHaveLength(1);
    await expect(page.locator("#import-notes")).toHaveText("2 import notes");
    await page.click("#import-notes");
    const listed = await page.evaluate(() => document.querySelector("#unsupported-list")!.textContent!.split("\n"));
    expect(listed).toHaveLength(2);
    expect(listed.filter((line) => DATES_LINE.test(line))).toHaveLength(1);
    expect(listed.filter((line) => ACTUALS_LINE.test(line))).toHaveLength(1);

    // The build marks the notes the generator reports by construction, and never the dates line.
    expect(sample.expectedNotes?.length).toBeGreaterThan(0);
    for (const mark of sample.expectedNotes!) expect(mark.startsWith("in-progress"), mark).toBe(false);
  });
}

test("office-fitout opens with the reader's one note, about its crews' assignments, kept out of the toolbar", async ({ page }) => {
  const sample = await openSample(page, "office-fitout");
  // 50 activities less the 3 milestones have a crew; the note is the reader's line as it stands on main (the names are read but not shown yet).
  expect(await page.evaluate(() => window.gantt!.unsupported!)).toEqual(["47 tasks have resource assignments; only the resource names were read, and no dates are affected."]);
  expect(sample.expectedNotes, "the build marks it expected").toEqual(["47 tasks have resource assignments"]);
  await expect(page.locator("#import-notes"), "the toolbar has no notice").toBeHidden();
});

/** Hold the first `held` reads of a file named `medium-5k.xer`: each waits for its own `releaseSample(n)`, in the order the page asks. Later reads go through. */
async function gateSampleReads(page: Page, held: number): Promise<void> {
  await page.addInitScript((held) => {
    const read = File.prototype.arrayBuffer;
    const gates: (() => void)[] = [];
    let reads = 0;
    (window as unknown as { releaseSample: (n: number) => void }).releaseSample = (n) => gates[n]?.();
    File.prototype.arrayBuffer = async function (this: File) {
      if (this.name === "medium-5k.xer" && (reads += 1) <= held) await new Promise<void>((resolve) => gates.push(resolve));
      return read.call(this);
    };
  }, held);
  await page.goto("/");
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, { timeout: 180_000 });
}

const pickSample = async (page: Page): Promise<void> => {
  await page.click("#open-button");
  await page.click('#sample-list button[data-sample-id="medium-5k"]');
};
const release = (page: Page, n: number): Promise<void> => page.evaluate((i) => (window as unknown as { releaseSample: (n: number) => void }).releaseSample(i), n);

test("a real file with the sample's name and size, opened while the sample is being read, keeps all of its own notes", async ({ page }) => {
  await gateSampleReads(page, 1);
  const file = SAMPLES.find((candidate) => candidate.id === "medium-5k") as Entry & { file: string };
  const sampleBytes = gunzipSync(readFileSync(new URL(`../public/samples/${file.file}`, import.meta.url))).length;
  await pickSample(page);
  await expect(page.locator("#open-info")).toContainText("Opening medium-5k.xer");

  // The ledger file (six notes, four of which start like the sample's expected ones), padded to the sample's size.
  const ledger = readFileSync(new URL("../../../packages/gantt/tests/fixtures/ledger-timeline.xer", import.meta.url));
  const padded = Buffer.concat([ledger, Buffer.alloc(sampleBytes - ledger.length, 0x20)]);
  await page.setInputFiles("#open-input", { name: "medium-5k.xer", mimeType: "application/octet-stream", buffer: padded });
  // The padding adds a note of its own, so the count is what the reader reported, and all of it shows.
  await page.waitForFunction(() => window.gantt!.opened?.name === "medium-5k.xer" && window.gantt!.unsupported!.length > 4, undefined, { timeout: 60_000 });
  const all = await page.evaluate(() => window.gantt!.unsupported!.length);
  await expect(page.locator("#import-notes")).toHaveText(`${all} import notes`);
  await release(page, 0);
  await page.waitForTimeout(1000);
  await expect(page.locator("#import-notes")).toHaveText(`${all} import notes`);
});

test("the same sample opened twice: the older read ending first does not take the newer open's exclusions", async ({ page }) => {
  await gateSampleReads(page, 2);
  await pickSample(page);
  await expect(page.locator("#open-info")).toContainText("Opening medium-5k.xer");
  await pickSample(page);
  // The older read ends first and is dropped as superseded; then the newer one ends.
  await release(page, 0);
  await page.waitForTimeout(1000);
  await release(page, 1);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(SAMPLES.find((candidate) => candidate.id === "medium-5k")!.rows)} rows`, { timeout: 120_000 });
  await expect(page.locator("#import-notes")).toHaveText("2 import notes");
});
