// The open-time half of the scripted measurement (`--open`): the statistics, the result block, the verdict
// and the page driving. Re-exported by `demos/50k/measure.mjs`, which the tests import.
import { resolve } from "node:path";

/** The open-time target, ms: the product's "instantly" (journey J1). */
export const OPEN_TARGET_MS = 2000;

/** The fields of one open's timings, in the order the page adds them up (packages/gantt/src/open-schedule.ts, `OpenTimings`). */
export const OPEN_PHASES = ["read", "paint", "copy", "parse", "build", "compute", "swap", "firstFrame", "total"];

/**
 * The Rust span (`OpenTimings.rustSpan`): Rust's own clock reads around the whole of `load_bytes`, taken outside the
 * three Rust phases. It is a check on them, not a phase: it is not added into `total`.
 */
export const OPEN_SPAN = "rustSpan";

/**
 * How far the sum of the three Rust phases may be from the Rust span, ms. The span's two clock reads sit outside the
 * phases', so the span also holds the cost of a clock call at each end (a `window().performance().now()` through
 * wasm-bindgen): 0.7 ms warm and 2.3 ms on a page's first open, measured in headless Chromium on hub. A wrong phase
 * is tens of milliseconds at the sizes this measures, so 3 ms is above the overhead and far below any error worth
 * catching.
 */
export const OPEN_SPAN_TOLERANCE_MS = 3;
export const OPEN_SUM_TOLERANCE_MS = 0.5;

/** How far the phases may be from `total`, ms: the clock's rounding. The windows are back to back, so it is exact otherwise. */

/** Nearest-rank percentile of an ascending list, the rule the page's own summaries use (packages/gantt/src/metrics.ts); NaN for none. */
export function percentile(sorted, p) {
  if (sorted.length === 0) return Number.NaN;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length, Math.max(1, rank)) - 1];
}

export function summarize(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return { count: sorted.length, p50: percentile(sorted, 50), p95: percentile(sorted, 95) };
}

/**
 * The `open` block of a result: one entry of `repeats` per recorded open (each the page's `OpenTimings`),
 * `p50` and `p95` of their `total`, and `phases`, the p50 and p95 of every phase. The warm-up open is kept
 * apart as `warmup` and is in none of the statistics. Throws if a run lacks a phase or holds a bad number,
 * because a missing or double-counted phase must stop the run, not average out.
 *
 * Three checks, because a sum that balances proves nothing by itself: the page derives `copy` as the call's
 * duration minus the Rust phases, so a Rust phase that under-reports lifts `copy` by the same amount and keeps the sum.
 *   1. every value, including `rustSpan`, is a finite number that is not negative (an invalid measurement is
 *      carried by the page as NaN or a negative, never as 0);
 *   2. `parse + build + compute` is within `OPEN_SPAN_TOLERANCE_MS` of `rustSpan`, which Rust measured outside those
 *      three phases: this is what catches a wrong Rust phase;
 *   3. the phases add up to `total` to within `OPEN_SUM_TOLERANCE_MS`: this catches a lost or double-counted window.
 */
export function openBlock({ file, bytes, tasks, warmup, repeats }) {
  if (repeats.length === 0) throw new Error("no open was recorded");
  for (const run of [warmup, ...repeats]) {
    for (const phase of [...OPEN_PHASES, OPEN_SPAN]) {
      if (typeof run?.[phase] !== "number" || !Number.isFinite(run[phase]) || run[phase] < 0) {
        throw new Error(`an open's ${phase} is ${JSON.stringify(run?.[phase])}; every phase must be a finite number of ms, not negative`);
      }
    }
    const inRust = run.parse + run.build + run.compute;
    if (Math.abs(inRust - run[OPEN_SPAN]) > OPEN_SPAN_TOLERANCE_MS) {
      throw new Error(
        `an open's Rust phases (parse, build, compute) add up to ${inRust.toFixed(2)} ms but Rust measured its own span as ${run[OPEN_SPAN].toFixed(2)} ms ` +
          `(more than ${OPEN_SPAN_TOLERANCE_MS} ms apart): a Rust phase is under- or over-reported`,
      );
    }
    const sum = OPEN_PHASES.filter((phase) => phase !== "total").reduce((total, phase) => total + run[phase], 0);
    if (Math.abs(run.total - sum) > OPEN_SUM_TOLERANCE_MS) {
      throw new Error(`an open's phases add up to ${sum.toFixed(2)} ms but its total is ${run.total.toFixed(2)} ms: a phase is missing or counted twice`);
    }
  }
  const totals = summarize(repeats.map((run) => run.total));
  return {
    file,
    bytes,
    tasks,
    warmup,
    repeats,
    p50: totals.p50,
    p95: totals.p95,
    phases: Object.fromEntries(OPEN_PHASES.map((phase) => {
      const { p50, p95 } = summarize(repeats.map((run) => run[phase]));
      return [phase, { p50, p95 }];
    })),
  };
}

/**
 * PASS or MISS of the open's p95 total against `OPEN_TARGET_MS`, only when the run is a release build on
 * hardware acceleration, as the page's own verdicts are; otherwise `verdict` is null and `note` says why.
 */
export function openVerdict(environment, block) {
  const reasons = [];
  if (environment?.hardwareAcceleration !== "on") reasons.push(`hardware acceleration is ${environment?.hardwareAcceleration ?? "unknown"}`);
  if (environment?.releaseBuild !== true) reasons.push("not a release build");
  const value = block.p95;
  const row = { name: `open: p95 total <= ${OPEN_TARGET_MS} ms`, value, limit: OPEN_TARGET_MS, unit: "ms" };
  if (reasons.length > 0) return { ...row, verdict: null, note: `no verdict (${reasons.join(", ")}): a smoke run, not a result` };
  return { ...row, verdict: value <= OPEN_TARGET_MS ? "PASS" : "MISS", note: null };
}

/**
 * Open `file` through the page's file input and return the page's record of the open (`window.gantt.opened`),
 * waiting for the first frame so its `timings` are complete. A file with several projects shows the picker; the
 * button for `project` is clicked. Throws with the page's own message when the page reports an error.
 */
export async function openOnce(page, file, project) {
  const before = await page.evaluate(() => window.gantt.status().epoch);
  await page.setInputFiles("#open-input", file);
  const settled = () =>
    page.waitForFunction(
      (epoch) => {
        const error = document.getElementById("open-error");
        if (error && !error.hidden) return { error: document.getElementById("open-error-text")?.textContent ?? "an error" };
        if (document.getElementById("picker")?.open) return { picker: true };
        const g = window.gantt;
        if (g.status().epoch === epoch + 1 && g.opened?.timings) return { opened: g.opened };
        return null;
      },
      before,
      { timeout: 120_000, polling: 100 },
    ).then((handle) => handle.jsonValue());
  let state = await settled();
  if (state.picker) {
    const choice = page.locator(`#picker-list button[data-project="${project}"]`);
    if ((await choice.count()) !== 1) throw new Error(`the file holds several projects and none has the id ${project}; pass --open-project <proj_id|all>`);
    await choice.click();
    state = await settled();
  }
  if (state.error) throw new Error(`the page could not open ${file}: ${state.error}`);
  return state.opened;
}

/** Open `file` once as a warm-up and `repeats` more times, and return the result's `open` block. */
export async function measureOpens(page, { file, repeats, project }) {
  const warmup = await openOnce(page, file, project);
  const runs = [];
  let last = warmup;
  for (let i = 0; i < repeats; i++) {
    last = await openOnce(page, file, project);
    runs.push(last.timings);
  }
  return openBlock({ file, bytes: last.bytes, tasks: last.tasks, warmup: warmup.timings, repeats: runs });
}

/**
 * Read `--open <file>`, `--open-repeats <n>` and `--open-project <p>` from the command line's arguments. Throws on a
 * flag given without its value (a trailing `--open` must not quietly turn the open measurement off), on a bad
 * repeat count, and on `--open-repeats` or `--open-project` without `--open`. `file` is undefined without `--open`.
 */
export function openArguments(args) {
  const has = (name) => args.includes(name);
  const value = (name) => {
    if (!has(name)) return undefined;
    const given = args[args.indexOf(name) + 1];
    if (given === undefined || given.startsWith("--")) throw new Error(`${name} needs a value, got ${given === undefined ? "nothing" : given}`);
    return given;
  };
  const file = value("--open");
  const repeatsText = value("--open-repeats");
  const project = value("--open-project");
  if (file === undefined && (repeatsText !== undefined || project !== undefined)) {
    throw new Error("--open-repeats and --open-project need --open <file>");
  }
  const repeats = Number(repeatsText ?? 5);
  if (!Number.isInteger(repeats) || repeats < 1) throw new Error(`--open-repeats needs a whole number of at least 1, got ${repeatsText}`);
  return { file: file === undefined ? undefined : resolve(file), repeats, project: project ?? "all" };
}
