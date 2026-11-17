import type { Page, TestInfo } from "@playwright/test";
import type { Status } from "../../src/api";
import { expect } from "./gantt";
import { dpr } from "./knobs";
import { AXIS, decodePng, rgb, type Png } from "./pixels";

export interface ViewBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DriftTracker {
  maxDrift: number;
  steps: number;
}

export interface PixelCensus {
  critical: number;
  normal: number;
  arrow: number;
  accent: number;
  axisText: number;
}

export interface PlacedLabel {
  task: number;
  text: string;
  placement: string;
  x: number;
  y: number;
  right: number;
  textW: number;
  gap: number;
  expect: string;
  minutes: number;
}

const nextFrame = (page: Page, k = 2): Promise<void> =>
  page.evaluate(
    (frameCount) =>
      new Promise<void>((done) => {
        const target = window.gantt!.status().frames + frameCount;
        const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
        requestAnimationFrame(wait);
      }),
    k,
  );

/** One ctrl+wheel step at page (px, py); returns the anchor error in px. */
export async function zoomStep(
  page: Page,
  box: ViewBox,
  drift: DriftTracker,
  px: number,
  py: number,
  dy: number,
): Promise<number> {
  const x = px - box.x;
  const t = await page.evaluate((at) => window.gantt!.app.time_at(at), x);
  await page.mouse.move(px, py);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, dy);
  await page.keyboard.up("Control");
  await nextFrame(page);
  const after = await page.evaluate((at) => window.gantt!.app.x_of(at), t);
  const error = Math.abs(after - x);
  drift.maxDrift = Math.max(drift.maxDrift, error);
  drift.steps++;
  return error;
}

export async function zoomUntil(
  page: Page,
  box: ViewBox,
  drift: DriftTracker,
  px: number,
  py: number,
  dy: number,
  level: string,
): Promise<void> {
  for (let i = 0; i < 200 && (await page.evaluate(() => window.gantt!.status())).level !== level; i++) {
    await zoomStep(page, box, drift, px, py, dy);
  }
  const s = await page.evaluate(() => window.gantt!.status());
  expect.soft(s.level === level, `reached the ${level} tick level (${s.from} .. ${s.to})`).toBe(true);
}

/** Count the view's five diagnostic colours, using the legacy tolerance of 6 per channel. The colour classes are the live theme's `crit-fill` and `crit-soft` (critical), `bar` and `bar-soft` (normal), `sel-edge` and `link` (gs#409), and axis text is a pixel of the axis band nearer `muted` than `axis`. A bar is its pastel body, with a saturated fill only where the task has actuals, so both colours count. */
export function census(png: Png, theme: Record<string, string>): PixelCensus {
  const [crit, critBody, bar, barBody, accent, link, label, axisBg] = [theme["crit-fill"], theme["crit-soft"], theme.bar, theme["bar-soft"], theme["sel-edge"], theme.link, theme.muted, theme.axis].map(rgb);
  // An axis-label pixel is nearer the label colour than the axis background (tl#1193: a fixed cut at 0x90 missed the
  // anti-aliased edges of a blue-grey label on the whiteprint axis).
  const dist = (r: number, g: number, b: number, [R, G, B]: readonly [number, number, number]): number => (r - R) ** 2 + (g - G) ** 2 + (b - B) ** 2;
  const near = (r: number, g: number, b: number, [R, G, B]: readonly [number, number, number]): boolean =>
    Math.abs(r - R) <= 6 && Math.abs(g - G) <= 6 && Math.abs(b - B) <= 6;
  const counts: PixelCensus = { critical: 0, normal: 0, arrow: 0, accent: 0, axisText: 0 };
  const band = Math.round(AXIS * dpr);
  for (let y = 0; y < png.height; y++) {
    for (let x = 0; x < png.width; x++) {
      const i = (y * png.width + x) * png.channels;
      const [r, g, b] = [png.px[i], png.px[i + 1], png.px[i + 2]];
      if (y >= band && (near(r, g, b, crit) || near(r, g, b, critBody))) counts.critical++;
      else if (y >= band && (near(r, g, b, bar) || near(r, g, b, barBody))) counts.normal++;
      else if (y >= band && near(r, g, b, accent)) counts.accent++;
      else if (y >= band && near(r, g, b, link)) counts.arrow++;
      else if (y < band && dist(r, g, b, label) < dist(r, g, b, axisBg)) counts.axisText++;
    }
  }
  return counts;
}

export async function shot(page: Page, testInfo: TestInfo, name: string): Promise<{
  name: string;
  status: Status;
  pixels: PixelCensus;
  png: Png;
  buffer: Buffer;
}> {
  await nextFrame(page);
  const buffer = await page.locator("#timeline").screenshot();
  await testInfo.attach(name, { body: buffer, contentType: "image/png" });
  const png = decodePng(buffer);
  const pixels = census(png, await page.evaluate(() => window.gantt!.theme()));
  const status = await page.evaluate(() => window.gantt!.status());
  console.log(
    `  ${name}: level ${status.level}, rows ${status.rows.join("-")}, bars ${status.bars} (${status.criticalBars} critical), arrows ${status.arrows}, pixels ${JSON.stringify(pixels)}`,
  );
  return { name, status, pixels, png, buffer };
}

/** Labels placed in the last frame, with each task's expected name and duration. */
export const placedLabels = (page: Page): Promise<PlacedLabel[]> =>
  page.evaluate(() => {
    const app = window.gantt!.app;
    const labels = app.test_bar_labels() as { task: number; text: string; placement: string; x: number; y: number; right: number; textW: number; gap: number }[];
    return labels.map((label) => ({
      ...label,
      expect: app.task_label(label.task),
      minutes: app.test_task_minutes(label.task),
    }));
  });

export const labelsOk = (labels: PlacedLabel[]): boolean =>
  labels.every(
    (label) =>
      label.text !== "" &&
      label.text === label.expect &&
      (label.placement === "inside" || label.placement === "right") &&
      label.x >= 0 &&
      label.right > label.x,
  ) && new Set(labels.map((label) => label.task)).size === labels.length;

const LEVELS = ["hour", "day", "week", "month", "year"];

/** The ctrl+wheel `deltaY` that moves the axis from level `from` toward `to`: in (-120) when `from` is the coarser level, out (+120) when it is the finer. A page opens fitted to its first screen (gs#821), so a spec cannot assume which side of its target level it starts on. */
export function zoomToward(from: string, to: string): number {
  return LEVELS.indexOf(from) < LEVELS.indexOf(to) ? 120 : -120;
}
