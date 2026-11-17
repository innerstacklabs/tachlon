import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";
import { dpr } from "./support/knobs";
import { AXIS, ROW, decodePng, type Png } from "./support/pixels";
import { zoomToward } from "./support/view";

// The two-tier header (gs#315, story 2 of gs#305): 48 px of week names over day letters at the weeks zoom and month names
// over weeks further out, a faint line down the body at every day, the data-date tag as a pill, and the task list's head
// as tall as the band so the rows stay in line. Sizes are `--gantt-*` tokens; colours come from the page's own theme.

type Rgb = readonly [number, number, number];

function rgb(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1, 7), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function at(png: Png, x: number, y: number): Rgb {
  const i = (Math.round(y * dpr) * png.width + Math.round(x * dpr)) * png.channels;
  return [png.px[i], png.px[i + 1], png.px[i + 2]];
}

function near(a: Rgb, b: Rgb, tol = 3): boolean {
  return Math.abs(a[0] - b[0]) <= tol && Math.abs(a[1] - b[1]) <= tol && Math.abs(a[2] - b[2]) <= tol;
}

/** CSS-pixel columns across the canvas where the pixel at `y` is one of `colours`. */
function columnsAt(png: Png, y: number, colours: Rgb[]): number[] {
  const out: number[] = [];
  for (let x = 0; x < png.width / dpr; x++) if (colours.some((c) => near(at(png, x, y), c))) out.push(x);
  return out;
}

/** Zoom in at row 6's bar until the tick level is `level`, then park that bar 150px in. */
async function zoomTo(g: Gantt, level: string): Promise<void> {
  const { page } = g;
  const box = await g.box();
  const t = await page.evaluate(() => window.gantt!.app.row_start(6) ?? 0);
  const px = box.x + (await page.evaluate((tt) => window.gantt!.app.x_of(tt), t)) + 4;
  const py = box.y + AXIS + 6 * ROW + ROW / 2;
  for (let i = 0; i < 200 && (await g.status()).level !== level; i++) {
    await page.mouse.move(px, py);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, zoomToward((await g.status()).level, level));
    await page.keyboard.up("Control");
    await g.nextFrame();
  }
  expect((await g.status()).level).toBe(level);
  for (let i = 0; i < 6; i++) {
    const x = await page.evaluate((tt) => window.gantt!.app.x_of(tt), t);
    if (Math.abs(x - 150) < 3) break;
    await page.evaluate((dx) => window.gantt!.app.wheel(0, 0, dx, 0, false), x - 150);
    await g.nextFrame();
  }
  await page.mouse.move(box.x + box.width - 5, box.y + box.height - 5);
  await g.nextFrame();
}

async function shot(page: Page): Promise<Png & { buf: Buffer }> {
  const buf = await page.locator("#timeline").screenshot();
  return { ...decodePng(buf), buf };
}

/** Pixels in the rows `y0` to `y1` (CSS px) that are not the axis colour, a grid line or a rule: the text. */
function ink(png: Png, y0: number, y1: number, axis: Rgb, lines: Rgb[]): number {
  let count = 0;
  for (let y = y0; y < y1; y++) {
    for (let x = 0; x < png.width / dpr; x++) {
      const c = at(png, x, y);
      if (!near(c, axis, 6) && !lines.some((l) => near(c, l, 6))) count++;
    }
  }
  return count;
}

test("the task list head is as tall as the axis band, so the rows stay in line", async ({ g, page }) => {
  await g.open();
  expect(AXIS, "the header is 48 px").toBe(48);
  const head = await page.locator("#table-head").boundingBox();
  const canvas = await g.box();
  expect(head!.height, "the head matches --gantt-axis-h").toBeCloseTo(AXIS, 1);
  expect((await g.status()).axisHeight).toBe(AXIS);
  const first = await page.locator("#table-rows .row").first().boundingBox();
  expect(first!.y - canvas.y, "the first table row starts where the first bar row does").toBeCloseTo(AXIS, 1);
  expect(first!.height).toBeCloseTo(ROW, 1);
  const token = await page.evaluate(() => getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-axis-h"));
  expect(token.trim()).toBe(`${AXIS}px`);
});

test("weeks zoom is week names over day letters with a faint line at every day; further out it is months over weeks", async ({
  g,
  page,
}, testInfo) => {
  await g.open();
  const theme = await page.evaluate(() => window.gantt!.theme());
  const [axis, grid, major] = [rgb(theme.axis), rgb(theme.grid), rgb(theme["grid-major"])];

  await zoomTo(g, "week");
  let status = await g.status();
  let png = await shot(page);
  await testInfo.attach("months-50k", { body: png.buf, contentType: "image/png" });
  const weekPx = status.pxPerMinute * 7 * 1440;
  expect(weekPx / 7, "months zoom: a day is under 14px").toBeLessThan(14);
  // A line down the body at every week; a row's top air is clear of bars.
  const y = AXIS + 4 * ROW + 1;
  const weeks = columnsAt(png, y, [grid, major]);
  expect(weeks.length, `lines at weeks: ${weeks.length} for ${(png.width / dpr / weekPx).toFixed(1)} weeks`).toBeGreaterThanOrEqual(
    Math.floor(png.width / dpr / weekPx) - 1,
  );
  expect(ink(png, 3, 21, axis, [grid, major]), "month names in the top tier").toBeGreaterThan(40);
  expect(ink(png, 27, 45, axis, [grid, major]), "week dates in the bottom tier").toBeGreaterThan(40);

  await zoomTo(g, "day");
  status = await g.status();
  png = await shot(page);
  await testInfo.attach("weeks-50k", { body: png.buf, contentType: "image/png" });
  const dayPx = status.pxPerMinute * 1440;
  expect(dayPx, "weeks zoom: a day is 14px or wider").toBeGreaterThanOrEqual(14);
  const days = columnsAt(png, y, [grid, major]);
  const visible = png.width / dpr / dayPx;
  expect(days.length, `lines at days: ${days.length} for ${visible.toFixed(1)} days`).toBeGreaterThanOrEqual(Math.floor(visible) - 2);
  expect(days.length, "no more lines than days").toBeLessThanOrEqual(Math.ceil(visible) + 2);
  // A day line is the faint grid token and the week lines the stronger one.
  expect(columnsAt(png, y, [major]).length, "a stronger line at every week").toBeGreaterThanOrEqual(Math.floor(visible / 7) - 1);
  expect(columnsAt(png, y, [grid]).length).toBeGreaterThan(columnsAt(png, y, [major]).length);
  expect(ink(png, 3, 21, axis, [grid, major]), "week names in the top tier").toBeGreaterThan(40);
  expect(ink(png, 27, 45, axis, [grid, major]), "day letters in the bottom tier").toBeGreaterThan(40);
});

test("the data-date tag is a pill in the lower tier", async ({ g, page }, testInfo) => {
  const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
  // The import notes are a toolbar notice, closed until it is clicked: nothing sits over the header.
  await expect(page.locator("#unsupported")).toBeHidden();
  await g.nextFrame(2);
  const x = (await g.status()).dataDateX!;
  const theme = await page.evaluate(() => window.gantt!.theme());
  const today = rgb(theme.today);
  const png = await shot(page);
  await testInfo.attach("data-date-pill", { body: png.buf, contentType: "image/png" });
  // The pill hangs to the right of the line, 18px tall and centred in the lower tier (24 to 48).
  const row = (yy: number): number[] => columnsAt(png, yy, [today]).filter((c) => c >= x - 2);
  const middle = row(36);
  // The tag text crosses the middle row, so measure from the first pill column to the last.
  expect(middle.at(-1)! - middle[0] + 1, "the pill is wide").toBeGreaterThanOrEqual(60);
  const left = middle[0];
  let top = 24;
  while (top < 48 && !near(at(png, left + 20, top), today)) top++;
  let bottom = 47;
  while (bottom > top && !near(at(png, left + 20, bottom), today)) bottom--;
  expect(bottom - top + 1, "the pill is 18px tall").toBeGreaterThanOrEqual(17);
  expect(bottom - top + 1).toBeLessThanOrEqual(19);
  expect(top, "centred in the lower tier").toBeGreaterThanOrEqual(26);
  expect(bottom).toBeLessThanOrEqual(45);
  // Its ends are round: the corner pixel of the box is not the pill's colour.
  expect(near(at(png, left + 2, top), today), "the corner is rounded").toBe(false);
  expect(near(at(png, left + 12, top), today), "the top edge is flat past the curve").toBe(true);
});
