import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { AXIS, ROW, countColour, decodePng, rgb } from "./support/pixels";
import { labelsOk, placedLabels, shot, zoomUntil, type DriftTracker, type ViewBox } from "./support/view";

// The zoom and pan journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P10). One 50k page is shared by the file;
// its four tests deliberately continue from the view left by the previous test.

const status = (page: Page) => page.evaluate(() => window.gantt!.status());

test.describe.serial("zoom and pan journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: ViewBox;
  let cx: number;
  let cy: number;
  let rowY: number;
  let barX: number;
  const drift: DriftTracker = { maxDrift: 0, steps: 0 };

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    const query = new URLSearchParams({ n: String(n) });
    if (backend === "webgl") query.set("backend", "webgl");
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    const bounds = await page.locator("#timeline").boundingBox();
    if (!bounds) throw new Error("#timeline has no bounding box");
    box = bounds;
    cx = box.x + box.width / 2;
    cy = box.y + AXIS + 10 * ROW;
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("years view shows the schedule, labels, arrows and summary bars", { tag: "@smoke" }, async ({}, testInfo) => {
    await zoomUntil(page, box, drift, cx, cy, 120, "year");
    const criticalRow = await page.evaluate(() => window.gantt!.app.test_first_critical_row() ?? null);
    expect.soft(criticalRow !== null, `the schedule has a critical task (first on row ${criticalRow})`).toBe(true);
    if (criticalRow !== null) {
      const es = await page.evaluate((row) => window.gantt!.app.row_start(row), criticalRow);
      const target = Math.max(0, (criticalRow - 5) * ROW);
      await page.mouse.move(cx, cy);
      let before = Infinity;
      for (let i = 0; i < 1000; i++) {
        const dy = target - (await status(page)).scrollY;
        const dx = (await page.evaluate((at) => window.gantt!.app.x_of(at), es!)) - box.width / 5;
        const off = Math.abs(dx) + Math.abs(dy);
        if (off < 1 || off >= before) break;
        before = off;
        const step = (distance: number): number => Math.sign(distance) * Math.min(Math.abs(distance), 40000);
        await page.mouse.wheel(step(dx), step(dy));
        await page.evaluate(
          () =>
            new Promise<void>((done) => {
              const targetFrame = window.gantt!.status().frames + 2;
              const wait = (): void => (window.gantt!.status().frames >= targetFrame ? done() : void requestAnimationFrame(wait));
              requestAnimationFrame(wait);
            }),
        );
      }
    }
    const sc = await status(page);
    expect.soft(sc.criticalBars > 0, `scrolled to rows ${sc.rows.join("-")}, ${sc.criticalBars} critical bars in view`).toBe(true);

    const years = await shot(page, testInfo, "1-years");
    expect.soft(years.pixels.critical > 20 && years.pixels.normal > 20, "years view shows critical and non-critical bars").toBe(true);
    expect.soft(years.pixels.axisText > 50, "years view shows axis labels").toBe(true);
    const yearLabels = await placedLabels(page);
    expect
      .soft(
        labelsOk(yearLabels),
        `years view places its labels cleanly (${yearLabels.length} of ${years.status.bars} bars named)`,
      )
      .toBe(true);
    // Names stop past the weeks zoom (gs#316).
    expect.soft(yearLabels.length, "years view names no bars").toBe(0);
    expect
      .soft(
        years.status.arrows > 0 && years.pixels.arrow > 20,
        `years view draws dependency arrows (${years.status.arrows} links, ${years.pixels.arrow} arrow pixels)`,
      )
      .toBe(true);
    const yearSummaryRows = await page.evaluate(
      ([first, last]) => JSON.parse(window.gantt!.app.table_rows(first, last)).filter((row: unknown[]) => row[5] !== 0).length,
      years.status.rows,
    );
    if (yearSummaryRows === 0) {
      if (n === 50000) {
        expect.soft(false, `summary bar colour check: no summary row in rows ${years.status.rows.join("-")}`).toBe(true);
      } else {
        console.log(`skip summary bar colour check: no summary row in rows ${years.status.rows.join("-")}`);
      }
    } else {
      const sumColour = (await page.evaluate(() => window.gantt!.theme())).sum;
      const summaryPixels = countColour(decodePng(years.buffer), rgb(sumColour), 2);
      expect
        .soft(
          summaryPixels >= 20 * yearSummaryRows,
          `years view draws summary bars (${yearSummaryRows} summary rows in view, ${summaryPixels} pixels within 2 per channel of the sum token ${sumColour}, want >= ${20 * yearSummaryRows}; measured 445 on 4 rows at 50000 tasks with the calm sum colour, gs#409; none of them text)`,
        )
        .toBe(true);
    }

    rowY = box.y + AXIS + 6 * ROW + ROW / 2;
    let foundX: number | null = null;
    for (let px = box.x + 2; px < box.x + box.width && foundX === null; px += 1) {
      await page.mouse.move(px, rowY);
      const hover = (await status(page)).hover;
      if (hover) {
        const mid = (hover.es + hover.ef) / 2;
        foundX = box.x + (await page.evaluate((at) => window.gantt!.app.x_of(at), mid));
      }
    }
    expect.soft(foundX !== null, "hover hit test finds a bar on row 6").toBe(true);
    barX = foundX ?? cx;
  });

  test("weeks view shows bars, axis labels and arrows", async ({}, testInfo) => {
    await zoomUntil(page, box, drift, barX, rowY, -120, "week");
    const weeks = await shot(page, testInfo, "2-weeks");
    expect.soft(weeks.pixels.critical + weeks.pixels.normal > 200, "weeks view shows bars").toBe(true);
    expect.soft(weeks.pixels.axisText > 50, "weeks view shows axis labels").toBe(true);
    expect
      .soft(
        weeks.status.arrows > 0 && weeks.pixels.arrow > 20,
        `weeks view draws dependency arrows (${weeks.status.arrows} links, ${weeks.pixels.arrow} arrow pixels)`,
      )
      .toBe(true);
  });

  test("hours view shows the cursor bar and its labels without anchor drift", async ({}, testInfo) => {
    await zoomUntil(page, box, drift, barX, rowY, -120, "hour");
    const hours = await shot(page, testInfo, "3-hours");
    expect.soft(hours.pixels.critical + hours.pixels.normal > 200, "hours view shows the bar under the cursor").toBe(true);
    expect.soft(hours.pixels.axisText > 50, "hours view shows axis labels").toBe(true);
    const hourLabels = await placedLabels(page);
    expect
      .soft(
        hourLabels.length > 0,
        `hours view places bar labels (${hourLabels.length}: ${hourLabels.map((l) => `${l.placement} "${l.text}"`).slice(0, 3).join(", ")})`,
      )
      .toBe(true);
    expect.soft(labelsOk(hourLabels), "hours view: each label is its task's name, inside or right, once per task").toBe(true);
    const { maxDrift, steps } = drift;
    expect
      .soft(maxDrift <= 1, `zoom anchor: max drift ${maxDrift.toFixed(4)} px over ${steps} wheel steps (limit 1 px)`)
      .toBe(true);
  });

  test("a drag pans the timeline and rows with the pointer", async ({}, testInfo) => {
    await zoomUntil(page, box, drift, barX, rowY, 120, "day");
    const before = await status(page);
    const panX = box.x + box.width - 100;
    const panY = box.y + AXIS / 2;
    const tAt = await page.evaluate((x) => window.gantt!.app.time_at(x), panX - box.x);
    await page.mouse.move(panX, panY);
    await page.mouse.down();
    for (let k = 1; k <= 10; k++) await page.mouse.move(panX - 30 * k, panY + 15 * k);
    await page.mouse.up();
    await page.evaluate(
      () =>
        new Promise<void>((done) => {
          const target = window.gantt!.status().frames + 2;
          const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
          requestAnimationFrame(wait);
        }),
    );
    const after = await status(page);
    const tx = await page.evaluate((at) => window.gantt!.app.x_of(at), tAt);
    expect
      .soft(
        Math.abs(tx - (panX - box.x - 300)) <= 1,
        `pan: instant moved with the pointer (${(tx - (panX - box.x)).toFixed(2)} px, want -300)`,
      )
      .toBe(true);
    expect
      .soft(
        Math.abs(after.scrollY - before.scrollY + 150) <= 0.5,
        `pan: rows scrolled ${after.scrollY - before.scrollY} px (want -150)`,
      )
      .toBe(true);
    const pan = await shot(page, testInfo, "4-pan");
    expect.soft(pan.pixels.critical + pan.pixels.normal > 200, "panned view shows bars").toBe(true);
  });
});
