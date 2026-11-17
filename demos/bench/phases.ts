// The input phases the driver runs on any side: vertical scroll, horizontal scroll and drag (load is
// `load.ts`, memory `memory.ts`). Input is real browser input only (Playwright's mouse and wheel),
// one step and then a wait for the page's next frame, with steps of the same size on every side; the frame
// recorder in `frames.ts` reads the gaps meanwhile.
//
// A scroll wheels until the side's probe reads its target, at the same step size and pace on every
// side, and records how many events that took. The target is a fixed distance, the same for every
// side and every schedule size: SCROLL_Y_VIEWS viewport heights, SCROLL_X_VIEWS widths, or the side's
// own scrollable range on that axis when that is shorter (`scrollTarget`). The distance scrolled is
// recorded (`scrolled`) and printed, so a shorter range stays visible. Each phase
// checks that its input did what it claims
// and says so in `verified` (and `why` when it did not): a scroll that did not reach its target or
// come back, a drag whose side never said it was
// committed. The drag keeps recording after the release until the side's `dragState` changes, so a
// side that commits after the release is timed until it has, and an unverified drag carries no frame
// numbers. Each runs
// under `capped`, which stops it at the limit through an AbortSignal that every step checks, so a
// phase that ran out of time sends no more input; the driver then closes the page and runs nothing
// more on it.

import { nextFrame, startRecorder, stopRecorder, summarise, type FrameStats } from "./frames.ts";
import { grabTarget } from "./grab.ts";
import type { DragTarget, ScrollTarget, Side } from "./side.ts";

export const PHASES = ["load", "scroll-y", "scroll-x", "drag", "memory"] as const;
export type Phase = (typeof PHASES)[number];

/** The wheel step: a sweep's length over this many, so a side that follows the wheel's delta arrives in this many events each way. */
export const SCROLL_STEPS = 120;
/** The vertical sweep: this many viewport heights (the driver's, not the side's) down the rows, and back. */
export const SCROLL_Y_VIEWS = 50;
/** The horizontal sweep: this many viewport widths (the driver's, not the side's) along the time axis, and back. */
export const SCROLL_X_VIEWS = 20;
/** The smallest px a side has moved per wheel event (DHTMLX, down). */
export const MIN_STEP_PX = 90;
/** The most wheel events a sweep of `target` px sends each way before it stops short: twice what the smallest step needs, and never fewer than twice SCROLL_STEPS, which a side that follows the wheel's delta takes on any range. */
export const wheelCap = (target: number) => 2 * Math.max(SCROLL_STEPS, Math.ceil(Math.abs(target) / MIN_STEP_PX));
/** The drag: this many days, in this many pointer moves. */
export const DRAG_DAYS = 10;
export const DRAG_STEPS = 60;
/** Frames recorded after the drag is committed, on every side. */
export const DRAG_AFTER_FRAMES = 10;
/** How long after the release the drag may take to commit (the side's `dragState` to change) before it counts as not moved. */
export const DRAG_COMMIT_MS = 30_000;

/** The few calls of a Playwright page and CDP session the phases use. */
export interface DrivenPage {
  goto(url: string, opts: { waitUntil: "commit"; timeout: number }): Promise<unknown>;
  waitForLoadState(state: "load"): Promise<void>;
  evaluate<T>(fn: string | (() => T | Promise<T>)): Promise<T>;
  evaluate<T, A>(fn: (arg: A) => T | Promise<T>, arg: A): Promise<T>;
  keyboard: { down(key: string): Promise<void>; up(key: string): Promise<void> };
  mouse: {
    move(x: number, y: number): Promise<void>;
    down(): Promise<void>;
    up(): Promise<void>;
    wheel(dx: number, dy: number): Promise<void>;
  };
}
export interface Cdp {
  send(method: string, params?: object): Promise<any>;
}

/**
 * Why a phase has no number: `timeout`, it ran past --dnf-s (or the page did not answer in that
 * time), the only cause the table prints as "did not finish"; `error`, a hook or the driver threw;
 * `not-ready`, the side's probe never saw the asked-for row count, which may be a wrong probe as
 * much as a slow side; `not-run`, an earlier phase did not finish; `harness`, the driver's own
 * reading (a memory sample over CDP or vmmap) did not answer, which says nothing of the side. The
 * table refuses a run with an `error`, `not-ready` or `harness` phase rather than print it as the
 * side's result.
 */
export type DnfCause = "timeout" | "error" | "not-ready" | "not-run" | "harness";

export interface Dnf {
  dnf: true;
  cause: DnfCause;
  /** Seconds spent before giving up. */
  afterS: number;
  why: string;
}
interface Checked {
  dnf: false;
  /** Whether the input did what the phase claims; `why` says what went wrong when it did not. */
  verified: boolean;
  why?: string;
}
export interface ScrollResult extends FrameStats, Checked {
  steps: number;
  /** px per wheel event. */
  stepPx: number;
  /** The wheel events sent out to the target and back to the start (`wheelTo`). */
  wheelEvents: { out: number; back: number };
  /** How far the sweep was meant to go, px: the fixed distance, or the side's own range when that is shorter. */
  target: number;
  /** How far the view went out, px (`turn - from`): the distance the table prints. */
  scrolled: number;
  /** Where the view was at the start, at the turn and at the end (the hook's `top` or `left`). */
  from: number;
  turn: number;
  back: number;
}
interface DragShape {
  days: number;
  steps: number;
  afterFrames: number;
  from: { x: number; y: number };
  to: { x: number; y: number };
  /** The dragged task's id in the benchmark dataset, null when the side cannot say: the table refuses a pair whose sides dragged different tasks. */
  task: string | null;
  /** The width of one day where the drag ran, px. */
  pxPerDay: number;
  /** The ctrl+wheel zoom steps and wheel scroll steps `grab.ts` took to bring the bar into reach. */
  zoomSteps: number;
  scrollSteps: number;
}
/** A committed drag: its frames from the press until DRAG_AFTER_FRAMES after the commit, and the ms from the release to the commit. */
export interface DragVerified extends FrameStats, DragShape {
  dnf: false;
  verified: true;
  commitMs: number;
}
/** A drag that changed nothing: no frame numbers. */
export interface DragUnverified extends DragShape {
  dnf: false;
  verified: false;
  why: string;
}
export type DragResult = DragVerified | DragUnverified;

/** The drag's dnf when it found no bar: it sent no input, so the page is still fit to read memory from. */
export const NO_BAR = "no bar on screen to drag";

/** Run a hook in the page. */
const hook = <T>(page: DrivenPage, fn: () => T) => page.evaluate(fn);

/** How close the outward scroll must land: 2 px, or 0.5% of a long sweep. */
const tolerance = (target: number) => Math.max(2, Math.abs(target) * 0.005);
/** How close the return must land, whatever the sweep's length: the start is a fixed place, so a scaled tolerance would pass a return hundreds of rows short. */
export const BACK_TOLERANCE_PX = 2;

/** Whether a sweep reached its target and came back, and why not. */
export function checkSweep(from: number, turn: number, back: number, target: number): { verified: boolean; why?: string } {
  if (target <= 0) return { verified: false, why: "nothing to scroll" };
  if (Math.abs(turn - from - target) > tolerance(target)) return { verified: false, why: `went ${turn - from} px of ${target}` };
  if (Math.abs(back - from) > BACK_TOLERANCE_PX) return { verified: false, why: `came back to ${back - from} px from the start` };
  return { verified: true };
}

/**
 * Wheel events of (dx, dy), each followed by a wait for the next frame and a read of the side's
 * probe, until the probe's `axis` is within `tol` of `goal` or past it, the view has not moved for
 * SCROLL_STEPS events (the side ignores the wheel), or `cap` events are sent. The same size and
 * pace for every side: one that moves a fixed step per event whatever the delta (DHTMLX reads
 * `wheelDelta`) just takes more of them. Returns the events sent and where the probe ended.
 */
async function wheelTo(page: DrivenPage, side: Side, axis: "top" | "left", dx: number, dy: number, goal: number, tol: number, at: number, cap: number, signal: AbortSignal): Promise<[number, number]> {
  const dir = Math.sign(dx + dy);
  let sent = 0;
  for (let still = 0; sent < cap && still < SCROLL_STEPS && (goal - at) * dir > tol; sent++) {
    signal.throwIfAborted();
    await page.mouse.wheel(dx, dy);
    await page.evaluate(nextFrame);
    const now = (await hook(page, side.scrollTarget))[axis];
    still = now === at ? still + 1 : 0;
    at = now;
  }
  return [sent, at];
}

/** The distance a sweep covers: `fixed` px, or what is left of the side's own range past `from` when that is shorter. The same rule for every side. */
export const sweepTarget = (fixed: number, range: number, from: number) => Math.min(fixed, Math.max(0, range - from));

/** Wheel over the side's scroll target out `fixed` px (or its own range, when shorter), in steps of a SCROLL_STEPSth of that one frame apart, until it is there, and back until it is at the start. */
async function sweep(page: DrivenPage, side: Side, axis: "top" | "left", fixed: number, refreshHz: number, signal: AbortSignal): Promise<ScrollResult> {
  const t: ScrollTarget = await hook(page, side.scrollTarget);
  const from = t[axis];
  const target = sweepTarget(fixed, axis === "top" ? t.maxTop : t.maxLeft, from);
  const stepPx = target / SCROLL_STEPS;
  const [dx, dy] = axis === "top" ? [0, stepPx] : [stepPx, 0];
  const cap = wheelCap(target);
  signal.throwIfAborted();
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2);
  await page.evaluate(startRecorder);
  const [out, turn] = await wheelTo(page, side, axis, dx, dy, from + target, tolerance(target), from, cap, signal);
  const [back] = await wheelTo(page, side, axis, -dx, -dy, from, BACK_TOLERANCE_PX, turn, cap, signal);
  await page.evaluate(nextFrame);
  const end = (await hook(page, side.scrollTarget))[axis];
  const gaps = await page.evaluate(stopRecorder);
  const checked = checkSweep(from, turn, end, target);
  return { dnf: false, ...checked, ...summarise(gaps, refreshHz), steps: SCROLL_STEPS, stepPx, wheelEvents: { out, back }, target, scrolled: turn - from, from, turn, back: end };
}

/** SCROLL_Y_VIEWS heights of the driver's viewport down the rows (or the whole range, when that is less), and back. */
export async function scrollY(page: DrivenPage, side: Side, refreshHz: number, viewportHeight: number, signal: AbortSignal): Promise<ScrollResult> {
  return sweep(page, side, "top", SCROLL_Y_VIEWS * viewportHeight, refreshHz, signal);
}

/** SCROLL_X_VIEWS widths of the driver's viewport along the time axis (or the whole range, when that is less), and back. */
export async function scrollX(page: DrivenPage, side: Side, refreshHz: number, viewportWidth: number, signal: AbortSignal): Promise<ScrollResult> {
  return sweep(page, side, "left", SCROLL_X_VIEWS * viewportWidth, refreshHz, signal);
}

/** The script that checks `dragState(target)` each frame until it differs from `before`; resolves to the ms from the release event (`ganttUpAt`, its time stamp) to then (page clock), or false at the deadline. */
export function commitScript(side: Side, target: DragTarget, before: string, waitMs: number): string {
  return `new Promise((resolve) => {
    const state = (${side.dragState.toString()});
    const t0 = window.ganttUpAt ?? performance.now();
    const limit = performance.now();
    const tick = () => {
      let now = ${JSON.stringify(before)};
      try { now = String(state(${JSON.stringify(target)})); } catch (e) {}
      if (now !== ${JSON.stringify(before)}) resolve(Math.round((performance.now() - t0) * 10) / 10);
      else if (performance.now() - limit > ${Math.max(0, Math.round(waitMs))}) resolve(false);
      else requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  })`;
}

/**
 * Press on the bar `grab.ts` picks (the same rule on every side), move it DRAG_DAYS later in DRAG_STEPS moves one frame apart, release, then keep
 * recording until the side's `dragState` changes (the drag is committed) or `commitLimitMs` passes,
 * and DRAG_AFTER_FRAMES frames more. A side that commits after the release is timed until it has.
 * A drag that never changed the state is unverified and carries no frame numbers: none is published.
 */
export async function drag(page: DrivenPage, side: Side, refreshHz: number, signal: AbortSignal, commitLimitMs = DRAG_COMMIT_MS): Promise<DragResult | Dnf> {
  const grab = await grabTarget(page, side, signal);
  const t = grab.target;
  if (t === null) return { dnf: true, cause: "error", afterS: 0, why: `${NO_BAR}: ${grab.why}` };
  const before = String(await page.evaluate(side.dragState, t));
  const to = { x: t.x + DRAG_DAYS * t.pxPerDay, y: t.y };
  const shape: DragShape = {
    days: DRAG_DAYS,
    steps: DRAG_STEPS,
    afterFrames: DRAG_AFTER_FRAMES,
    from: { x: t.x, y: t.y },
    to,
    task: t.datasetId ?? null,
    pxPerDay: t.pxPerDay,
    zoomSteps: grab.zoomSteps,
    scrollSteps: grab.scrollSteps,
  };
  signal.throwIfAborted();
  await page.mouse.move(t.x, t.y);
  await page.evaluate(startRecorder);
  await page.mouse.down();
  for (let i = 1; i <= DRAG_STEPS; i++) {
    signal.throwIfAborted();
    await page.mouse.move(t.x + ((to.x - t.x) * i) / DRAG_STEPS, t.y);
    await page.evaluate(nextFrame);
  }
  // A side that reschedules inside the release's own handler is timed from the release event, not from after it.
  await page.evaluate(() => addEventListener("mouseup", (e) => ((window as unknown as { ganttUpAt: number }).ganttUpAt = e.timeStamp), { capture: true, once: true }));
  await page.mouse.up();
  const commitMs = await page.evaluate<number | false>(commitScript(side, t, before, commitLimitMs));
  signal.throwIfAborted();
  for (let i = 0; i < DRAG_AFTER_FRAMES; i++) await page.evaluate(nextFrame);
  const gaps = await page.evaluate(stopRecorder);
  if (commitMs === false) {
    return { dnf: false, verified: false, why: `the drag state stayed ${JSON.stringify(before)} for ${commitLimitMs / 1000} s after the release: no bar moved`, ...shape };
  }
  return { dnf: false, verified: true, ...summarise(gaps, refreshHz), commitMs, ...shape };
}

/**
 * `run`, or a dnf when it takes more than `limitMs` or throws (with the error as `why`). On the
 * limit the signal is aborted, so a run that checks it sends no more input.
 */
export async function capped<T>(run: (signal: AbortSignal) => Promise<T>, limitMs: number): Promise<T | Dnf> {
  const ac = new AbortController();
  const t0 = Date.now();
  let timer: NodeJS.Timeout | undefined;
  const late = new Promise<Dnf>((resolve) => {
    timer = setTimeout(() => {
      ac.abort();
      resolve({ dnf: true, cause: "timeout", afterS: limitMs / 1000, why: "the phase did not finish" });
    }, limitMs);
  });
  try {
    return await Promise.race([run(ac.signal), late]);
  } catch (e) {
    ac.abort();
    return { dnf: true, cause: "error", afterS: Math.round(Date.now() - t0) / 1000, why: `error: ${e instanceof Error ? e.message.split("\n")[0] : String(e)}` };
  } finally {
    clearTimeout(timer);
  }
}
