import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product";
import { toMspdi } from "../samples/office-fitout";
import { readSamplesJson } from "./samples-file";
import { pageErrors, trackLogs, waitReady, waitSampleSettled } from "./support/page";
import { ownXer } from "./support/xer";

// The hosted app's first ten seconds (gs#821): it opens on the office fit-out, its bars fill the chart, and the banner and the
// toolbar name the product. Measured at 1280 x 1000.

test.use({ viewport: { width: 1280, height: 1000 } });

const SAMPLES = readSamplesJson<{ samples: { id: string; rows: number }[] }>().samples;
const rowsOf = (id: string): number => SAMPLES.find((sample) => sample.id === id)!.rows;

// The office fit-out opens with a saved baseline set (gs#703), which adds Clear baseline to the toolbar. The specs that use the fit-out
// below test the no-baseline layout, so each clears it once after the file opens.
async function clearBaseline(page: Page): Promise<void> {
  await page.locator("#baseline-clear").click();
  await expect(page.locator("#baseline-clear")).toBeHidden();
}

test("with no ?sample= the page opens on the office fit-out, and the name shows once, in the toolbar", async ({ page }) => {
  const logs = trackLogs(page);
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await clearBaseline(page);
  await expect(page.locator("#row-count")).toHaveText(`${rowsOf("office-fitout")} rows`);
  await expect(page.locator("#table-rows")).toContainText("Site survey and measured drawings");
  await expect(page.locator("#toolbar .tb-brand")).toHaveText(PRODUCT_NAME);
  await expect(page.getByText(PRODUCT_NAME, { exact: true }), "the name is on screen once").toHaveCount(1);
  await expect(page.getByText(/Drawing with/), "no renderer readout on the page").toHaveCount(0);
  await expect(page.locator("#toolbar")).toHaveAttribute("title", /^Drawing with (WebGPU|WebGL2)$/);
  expect(pageErrors(logs), "the page logs no error").toEqual([]);
});

test("at 1280 px every task name on the office fit-out's first screen shows whole (gs#703 W3)", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await clearBaseline(page);
  await page.waitForFunction(() => document.querySelectorAll("#table-rows .row > .name").length > 20);
  const seen = await page.evaluate(() => {
    const names = [...document.querySelectorAll<HTMLElement>("#table-rows .row > .name")];
    return {
      count: names.length,
      cut: names.filter((name) => name.scrollWidth > name.clientWidth).map((name) => `${name.textContent} (${name.scrollWidth} > ${name.clientWidth})`),
    };
  });
  expect(seen.count, "the first screen shows the fit-out's rows").toBeGreaterThan(20);
  expect(seen.cut, "no task name is cut by an ellipsis").toEqual([]);
});

test("?sample=demo-50k still opens the generated demo", async ({ page }) => {
  await page.goto("/?sample=demo-50k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  expect(await page.evaluate(() => window.gantt!.status().n)).toBe(rowsOf("demo-50k"));
});

/**
 * The bars the last frame drew, against the canvas. A bar the view clips ends at the canvas edge, so a right edge under the width
 * is a real finish inside the view; the fit leaves a margin, so it must also stop short of the edge.
 */
async function barsFill(page: Page): Promise<{ left: number; right: number; width: number }> {
  const frames = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((from) => window.gantt!.status().frames >= from + 4, frames);
  return page.evaluate(() => {
    const bars = window.gantt!.app.test_bar_rects() as { x: number; width: number }[];
    return { left: Math.min(...bars.map((bar) => bar.x)), right: Math.max(...bars.map((bar) => bar.x + bar.width)), width: window.gantt!.status().surface[0] };
  });
}

function expectFilled(view: { left: number; right: number; width: number }, what: string): void {
  expect(view.left, `${what}: the first bar starts in the first tenth`).toBeLessThan(view.width / 10);
  expect(view.right, `${what}: the last bar ends in the last tenth`).toBeGreaterThan((view.width * 9) / 10);
  expect(view.right, `${what}: the real finish is inside the view, not clipped at its edge`).toBeLessThan(view.width - 1);
}

test("a file that adds or drops the ID column is fitted to the canvas it ends up with, at 1280 px", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await clearBaseline(page);
  // Clearing the baseline is an unsaved change, so each open below asks to confirm; this test opens files, so it accepts.
  page.on("dialog", (dialog) => void dialog.accept());
  // `Status.epoch` counts opens, and the fit-out sample is itself a file open, so each open waits for one more than the epoch
  // it started at. A fixed number (1, then 2) matched the epoch the page already had, so the wait passed before the file was in.
  const open = async (name: string, buffer: Buffer): Promise<void> => {
    const from = await page.evaluate(() => window.gantt!.status().epoch);
    await page.setInputFiles("#open-input", { name, mimeType: "application/octet-stream", buffer });
    await page.waitForFunction((n) => window.gantt!.status().epoch === n, from + 1, { timeout: 60_000 });
  };
  expectFilled(await barsFill(page), "the office fit-out as it loads");
  // A px --gantt-table-w, as the #split divider sets one (or a host does), is the width without the ID column: the column then widens the
  // table and narrows the canvas (gs#866: only the default split, half the box, holds the column inside its half).
  await page.evaluate(() => document.getElementById("app")!.style.setProperty("--gantt-table-w", "640px"));
  await page.waitForFunction(() => window.gantt!.status().frames > 0);
  const before = (await page.locator("#table").boundingBox())!.width;
  await open("own.xer", ownXer());
  await expect(page.locator("#table")).toHaveAttribute("data-ids", "true");
  expect((await page.locator("#table").boundingBox())!.width, "the ID column widens the table, so the canvas is narrower").toBeGreaterThan(before);
  expectFilled(await barsFill(page), "an XER with IDs (the canvas narrows)");
  await open("office-fitout.xml", Buffer.from(toMspdi(), "utf8"));
  await expect(page.locator("#table")).not.toHaveAttribute("data-ids", "true");
  expectFilled(await barsFill(page), "the office fit-out file (the canvas widens)");
});

for (const sample of ["demo-50k", "office-fitout"]) {
  test(`${sample} opens zoomed so the first screen's bars fill the chart`, async ({ page }) => {
    await page.goto(`/?sample=${sample}`);
    await waitReady(page, { frames: 10, timeout: 180_000 });
    await waitSampleSettled(page, 120_000);
    if (sample === "office-fitout") await clearBaseline(page);
    const frames = await page.evaluate(() => window.gantt!.status().frames);
    await page.waitForFunction((from) => window.gantt!.status().frames >= from + 3, frames);
    const view = await page.evaluate(() => {
      const bars = window.gantt!.app.test_bar_rects() as { x: number; width: number }[];
      return { left: Math.min(...bars.map((bar) => bar.x)), right: Math.max(...bars.map((bar) => bar.x + bar.width)), width: window.gantt!.status().surface[0] };
    });
    expectFilled(view, sample);
  });
}

test("scrolling the table ends the opening fit: moving the divider afterwards keeps the time origin and the zoom", async ({ page }) => {
  await page.goto("/?sample=demo-50k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const view = (): Promise<{ from: number; scale: number }> =>
    page.evaluate(() => ({ from: window.gantt!.status().t[0], scale: window.gantt!.status().pxPerMinute }));
  const opened = await view();
  await page.evaluate(() => {
    document.getElementById("table-body")!.scrollTop = 2400;
  });
  await page.waitForFunction(() => window.gantt!.status().scrollY > 0);
  await page.locator("#split").focus();
  await page.keyboard.press("ArrowRight");
  const frames = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((from) => window.gantt!.status().frames >= from + 4, frames);
  const after = await view();
  expect(after.scale, "the zoom is the one the page opened with").toBe(opened.scale);
  expect(after.from, "the time origin is the one the page opened with").toBe(opened.from);
});

test("the office fit-out opens with its data date, and its started activities show their progress", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#data-date"), "the data-date line's tag is shown").toBeVisible();
  // The MS Project reader applies a task's actual start, actual finish and remaining duration (gs#860), so the sample's
  // started activities give progress on the page.
  expect(await page.evaluate(() => window.gantt!.app.has_progress()), "the MS Project file gives progress").toBe(true);
  await expect(page.locator("#table"), "the progress columns are shown").toHaveAttribute("data-progress", "true");
});

// The first-screen line (gantt#1280): what the chart is and how to open your own file; it can be dismissed and stays dismissed.
test("the first-screen line shows on first open, and stays hidden after it is dismissed, also on reload", async ({ page }) => {
  await page.goto("/");
  const line = page.locator("#first-screen-line");
  await expect(line).toBeVisible();
  await expect(line).toContainText("P6 XER or MS Project XML");
  await expect(line).toContainText("save as XML first");
  await expect(line, "the line makes no promise about where the file stays").not.toContainText(/stays? (in|on)/i);
  await line.getByRole("button", { name: "Dismiss" }).click();
  await expect(line).toBeHidden();
  await page.reload();
  await waitReady(page, { frames: 1, timeout: 180_000 });
  await expect(line, "dismissed stays dismissed on reload").toBeHidden();
});

test("the first-screen line goes when you open your own file with the chooser", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 1, timeout: 180_000 });
  await expect(page.locator("#first-screen-line")).toBeVisible();
  page.on("dialog", (dialog) => void dialog.accept());
  await page.setInputFiles("#open-input", { name: "own.xer", mimeType: "application/octet-stream", buffer: ownXer() });
  await expect(page.locator("#first-screen-line")).toHaveCount(0);
});

test("with the first-screen line open at 1280 x 800, it covers neither the legend row nor the baseline note", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const line = page.locator("#first-screen-line");
  await expect(line).toBeVisible();
  const note = page.locator("[data-part=baseline-summary]");
  await expect(note, "the fit-out opens with a baseline, so its note shows").toContainText("vs baseline");
  const lineTop = (await line.boundingBox())!.y;
  for (const part of [page.locator("[data-part=legend]"), note]) {
    const box = (await part.boundingBox())!;
    expect(box.y + box.height, "the line starts below the part").toBeLessThanOrEqual(lineTop);
  }
});
