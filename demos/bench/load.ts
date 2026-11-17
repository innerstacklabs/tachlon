// The load phase: from the navigation to the frame after the side's `ready` first holds.
//
// Both ends are read on the page's clock: the start is the navigated document's
// `performance.timeOrigin`, the end the `requestAnimationFrame` timestamp of the frame after the one
// whose check passed, plus that same origin's epoch. The check runs once per frame in the page, so
// every side is timed the same way whatever its probe reads. A page may reload itself after a first
// ready frame (ours does when WebGPU fails and it falls back to WebGL2): the document is marked, and
// if after SETTLE_MS it was replaced or is no longer ready, the wait starts again. Load ends at the
// ready frame of the page the other phases drive.
//
// Load is split by the two marks every side's page sets (`side.ts`): `libraryMs`, from the navigation
// to `library-ready` (the component's code loaded and able to run), and `dataToReadyMs`, from
// `data-start` (the page starts fetching the schedule) to the ready frame. Each is null when the page
// did not set its mark. A side with a `spotCheck` then has a sample of its dates checked against the
// schedule's; a failed check makes load a dnf, so a broken setup gives no number at all. A side with
// `settled` (one whose ready frame can come before its own load work is done, such as a schedule
// shown from saved dates while the engine still computes) is then waited for, and `settledMs` (from the
// navigation) and `dataToSettledMs` (from `data-start`) say when it was done: the input phases never
// absorb load work. The spot check runs after that wait.

import type { Dnf, DrivenPage } from "./phases.ts";
import type { Side } from "./side.ts";

/** After the first ready frame, the page must stay the same document and ready this long. */
export const SETTLE_MS = 1000;

export interface LoadResult {
  dnf: false;
  /** From the navigation to the frame after the first ready one, ms, on the page's clock. */
  ms: number;
  /** From the navigation to the page's `library-ready` mark, ms; null without the mark. */
  libraryMs: number | null;
  /** From the page's `data-start` mark to the ready frame, ms; null without the mark. */
  dataToReadyMs: number | null;
  /** Whether the side checked a sample of its dates (it has a `spotCheck`, and it passed). */
  spotChecked: boolean;
  /** From the navigation to when the side's `settled` says its load work finished, ms; null for a side without the hook. */
  settledMs: number | null;
  /** From the page's `data-start` mark to that same moment, ms; null without the hook or the mark. */
  dataToSettledMs: number | null;
}

/** Each mark's time as epoch ms on the page's clock (its first mark of that name), or null. */
export interface Marks {
  library: number | null;
  data: number | null;
}
export const MARKS_SCRIPT = `(() => {
  const at = (name) => {
    const m = performance.getEntriesByName(name, "mark")[0];
    return m === undefined ? null : performance.timeOrigin + m.startTime;
  };
  return { library: at("library-ready"), data: at("data-start") };
})()`;

const round = (ms: number) => Math.round(ms * 10) / 10;

/** The load result from the navigation, the ready frame, the marks and when the side settled (null without `settled`), all epoch ms on the page's clock. */
export function splitLoad(origin: number, ready: number, marks: Marks, spotChecked: boolean, settled: number | null = null): LoadResult {
  return {
    dnf: false,
    ms: round(ready - origin),
    libraryMs: marks.library === null ? null : round(marks.library - origin),
    dataToReadyMs: marks.data === null ? null : round(ready - marks.data),
    spotChecked,
    settledMs: settled === null ? null : round(settled - origin),
    dataToSettledMs: settled === null || marks.data === null ? null : round(settled - marks.data),
  };
}

/** The script that resolves, once a frame, to the epoch ms the side's `settled` reports, or false at the deadline. */
export function settledScript(side: Side, waitMs: number): string {
  return `new Promise((resolve) => {
    const settled = (${side.settled!.toString()});
    const end = performance.now() + ${Math.max(0, Math.round(waitMs))};
    const tick = () => {
      let at = null;
      try { at = settled(); } catch (e) { at = null; }
      if (typeof at === "number") resolve(performance.timeOrigin + at);
      else if (performance.now() > end) resolve(false);
      else requestAnimationFrame(tick);
    };
    tick();
  })`;
}

const isTimeout = (e: unknown) => e instanceof Error && /timeout/i.test(e.name + e.message);
const isReload = (e: unknown) => /context was destroyed|navigat|target closed/i.test(String(e));

/** The script that checks `ready(rows)` each frame and resolves to the epoch ms of the frame after it holds, or false at the deadline. */
export function readyScript(side: Side, rows: number, waitMs: number): string {
  return `new Promise((resolve) => {
    const ready = (${side.ready.toString()});
    const end = performance.now() + ${Math.max(0, Math.round(waitMs))};
    const tick = () => {
      let ok = false;
      try { ok = ready(${rows}) === true; } catch (e) { ok = false; }
      if (ok) requestAnimationFrame((t) => resolve(performance.timeOrigin + t));
      else if (performance.now() > end) resolve(false);
      else requestAnimationFrame(tick);
    };
    tick();
  })`;
}

/** The epoch ms (page clock) of the frame after `ready` first holds, or undefined when `stop` passes. */
async function readyFrame(page: DrivenPage, side: Side, rows: number, stop: number): Promise<number | undefined> {
  for (;;) {
    const left = stop - Date.now();
    if (left <= 0) return undefined;
    try {
      const at = await page.evaluate<number | false>(readyScript(side, rows, left));
      return at === false ? undefined : at;
    } catch (e) {
      // A reload destroys the context under the wait; wait on the new document.
      if (!isReload(e)) throw e;
      await page.waitForLoadState("load").catch(() => undefined);
    }
  }
}

/** Navigate to `url` and wait for the side's first ready frame with `rows` tasks, up to `limitMs`. */
export async function load(page: DrivenPage, side: Side, url: string, rows: number, limitMs: number): Promise<LoadResult | Dnf> {
  const stop = Date.now() + limitMs;
  const dnf = (cause: Dnf["cause"], why: string): Dnf => ({ dnf: true, cause, afterS: limitMs / 1000, why });
  try {
    await page.goto(url, { waitUntil: "commit", timeout: limitMs });
  } catch (e) {
    if (isTimeout(e)) return dnf("timeout", "the page did not answer");
    throw e;
  }
  const origin = await page.evaluate<number>("performance.timeOrigin");
  const still = `(() => window.__benchLoaded === true && (${side.ready.toString()})(${rows}) === true)()`;
  for (;;) {
    const at = await readyFrame(page, side, rows, stop);
    if (at === undefined) return dnf("not-ready", `no ready frame with ${rows} tasks`);
    const marked = await page.evaluate("window.__benchLoaded = true").then(() => true, () => false);
    await new Promise((r) => setTimeout(r, SETTLE_MS));
    if (marked && (await page.evaluate(still).catch(() => false)) === true) {
      await page.evaluate("delete window.__benchLoaded");
      const marks = await page.evaluate<Marks>(MARKS_SCRIPT);
      let settled: number | null = null;
      if (side.settled !== undefined) {
        // The input phases start only once the side's own load work is done, so they absorb none of it.
        const done = await page.evaluate<number | false>(settledScript(side, Math.max(0, stop - Date.now())));
        if (done === false) return dnf("timeout", "the side's work after its ready frame did not finish");
        settled = done;
      }
      // Checked once the side's work is done, so a side that drew saved dates is checked as it ends up.
      let spotChecked = false;
      if (side.spotCheck !== undefined) {
        const wrong = await page.evaluate<string | null>(side.spotCheck as () => Promise<string | null>);
        if (wrong !== null) return { dnf: true, cause: "error", afterS: 0, why: `spot check: ${wrong}` };
        spotChecked = true;
      }
      return splitLoad(origin, at, marks, spotChecked, settled);
    }
    if (Date.now() >= stop) return dnf("not-ready", `no ready frame with ${rows} tasks`);
    await page.waitForLoadState("load").catch(() => undefined);
  }
}
