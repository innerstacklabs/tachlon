// Which bar the drag phase presses on, chosen by the driver by one rule for every side.
//
// A side only reports the task bars it draws (`leafBars`: boxes on the page, summaries left out);
// the driver picks among them. The bar is the first one fully inside the side's scroll target, in
// row order (top, then left). When no bar is in view, the wheel brings the first one reported into
// it; when the bar is narrower than GRAB_PX, ctrl+wheel over its centre zooms the time axis in on it.
// Each step is the same input on every side, then a wait for the next frame, and the pick is made
// again, up to GRAB_STEPS steps. While it zooms, a listener on the window stops the browser's own
// page zoom, so a side that leaves ctrl+wheel to the browser is not zoomed as a page; a zoom step that
// that leaves the width of a day as it was ends the search. After any step the driver waits until the bars the side
// reports stay the same for QUIET_MS, as load's settle wait does, so the re-layout a step set off
// is not in the drag's frames. None of it is recorded: the frame recorder starts at the press.

import { nextFrame } from "./frames.ts";
import { SETTLE_MS } from "./load.ts";
import type { Box, DragTarget, LeafBar, LeafBars, Side } from "./side.ts";

/** The narrowest bar the drag presses on, CSS px: wide enough to press its middle, clear of its ends. */
export const GRAB_PX = 6;
/** The wheel delta of one zoom step (ctrl+wheel up, one mouse notch). */
export const ZOOM_DY = -100;
/** The most wheel steps (scroll or zoom) the driver takes to bring a bar into reach. */
export const GRAB_STEPS = 30;
/** How long the side's bars must stay the same after a step before the drag starts, ms. */
export const QUIET_MS = SETTLE_MS;
/** How many QUIET_MS waits the bars get to stop moving. */
const QUIET_TRIES = 10;
/** How far inside the scroll target a bar scrolled into view lands, px. */
const MARGIN = 20;

/** The page calls the pick uses (a subset of Playwright's page). */
export interface GrabPage {
  evaluate<T>(fn: () => T | Promise<T>): Promise<T>;
  keyboard: { down(key: string): Promise<void>; up(key: string): Promise<void> };
  mouse: { move(x: number, y: number): Promise<void>; wheel(dx: number, dy: number): Promise<void> };
}

/** The bar picked, or null and why not, and the steps taken to reach it. */
export interface Grab {
  target: DragTarget | null;
  why?: string;
  zoomSteps: number;
  scrollSteps: number;
}

const inside = (b: Box, view: Box) => b.x >= view.x && b.y >= view.y && b.x + b.width <= view.x + view.width && b.y + b.height <= view.y + view.height;
const rowOrder = (a: Box, b: Box) => a.y - b.y || a.x - b.x;

/** The first bar fully inside `view`, in row order; null when none is. */
export function pickBar(bars: LeafBar[], view: Box): LeafBar | null {
  return bars.filter((b) => inside(b, view)).sort(rowOrder)[0] ?? null;
}

/** What the driver does next: drag `bar`, zoom in on it, scroll by (dx, dy), or give up. */
export type GrabStep = { kind: "drag"; bar: LeafBar } | { kind: "zoom"; bar: LeafBar } | { kind: "scroll"; dx: number; dy: number } | { kind: "none" };

/** The next step for these bars in this view, by the rule above. */
export function nextStep(bars: LeafBar[], view: Box): GrabStep {
  const bar = pickBar(bars, view);
  if (bar !== null) return bar.width >= GRAB_PX ? { kind: "drag", bar } : { kind: "zoom", bar };
  const first = [...bars].sort(rowOrder)[0];
  if (first === undefined) return { kind: "none" };
  // Bring its top-left corner MARGIN px inside the view's; the wheel only moves what lies outside.
  const off = (at: number, size: number, from: number, room: number) => (at >= from && at + size <= from + room ? 0 : at - from - MARGIN);
  return { kind: "scroll", dx: off(first.x, first.width, view.x, view.width), dy: off(first.y, first.height, view.y, view.height) };
}

/** In the page: stop the browser's own ctrl+wheel page zoom, after the side's listeners have had the event. */
export function guardZoom(): void {
  const w = window as any;
  w.benchZoomGuard ??= (e: WheelEvent) => {
    if (e.ctrlKey) e.preventDefault();
  };
  window.addEventListener("wheel", w.benchZoomGuard, { passive: false });
}

/** In the page: take the guard away again. */
export function unguardZoom(): void {
  const w = window as any;
  if (w.benchZoomGuard !== undefined) window.removeEventListener("wheel", w.benchZoomGuard);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Wait until the bars the side reports stay the same for QUIET_MS; false when they never do. */
async function quiet(page: GrabPage, side: Side, signal: AbortSignal): Promise<boolean> {
  for (let i = 0; i < QUIET_TRIES; i++) {
    const before = JSON.stringify(await page.evaluate(side.leafBars));
    await sleep(QUIET_MS);
    signal.throwIfAborted();
    await page.evaluate(nextFrame);
    if (JSON.stringify(await page.evaluate(side.leafBars)) === before) return true;
  }
  return false;
}

/** Pick the bar to drag, scrolling or zooming the same way on every side until one is in reach. */
export async function grabTarget(page: GrabPage, side: Side, signal: AbortSignal): Promise<Grab> {
  const g: Grab = { target: null, zoomSteps: 0, scrollSteps: 0 };
  await page.evaluate(guardZoom);
  try {
    let zoomedAt: number | null = null;
    let moved = false;
    for (;;) {
      signal.throwIfAborted();
      const { bars, pxPerDay }: LeafBars = await page.evaluate(side.leafBars);
      const view: Box = await page.evaluate(side.scrollTarget);
      const step = nextStep(bars, view);
      if (step.kind === "drag" && moved) {
        // Pick again once what the steps set off has settled: what it settled to is what is dragged.
        if (!(await quiet(page, side, signal))) return { ...g, why: `the bars still moved ${(QUIET_TRIES * QUIET_MS) / 1000} s after the last step` };
        moved = false;
        continue;
      }
      if (step.kind === "drag") {
        const { bar } = step;
        const target: DragTarget = { x: bar.x + bar.width / 2, y: bar.y + bar.height / 2, pxPerDay };
        if (bar.id !== undefined) target.id = bar.id;
        if (bar.datasetId !== undefined) target.datasetId = bar.datasetId;
        return { ...g, target };
      }
      if (step.kind === "none") return { ...g, why: "the side reports no task bar" };
      if (g.zoomSteps + g.scrollSteps === GRAB_STEPS) return { ...g, why: `no task bar in reach after ${GRAB_STEPS} steps` };
      if (step.kind === "zoom") {
        // A bar's width may be whole pixels and not grow on one notch; the width of a day does.
        if (zoomedAt !== null && pxPerDay <= zoomedAt) return { ...g, why: "ctrl+wheel did not zoom the time axis in: the side does not zoom on it" };
        zoomedAt = pxPerDay;
        await page.mouse.move(step.bar.x + step.bar.width / 2, step.bar.y + step.bar.height / 2);
        await page.keyboard.down("Control");
        await page.mouse.wheel(0, ZOOM_DY);
        await page.keyboard.up("Control");
        g.zoomSteps++;
      } else {
        await page.mouse.move(view.x + view.width / 2, view.y + view.height / 2);
        await page.mouse.wheel(step.dx, step.dy);
        g.scrollSteps++;
      }
      moved = true;
      await page.evaluate(nextFrame);
      await page.evaluate(nextFrame);
    }
  } finally {
    await page.evaluate(unguardZoom);
  }
}
