// The numbers of a measured run (plan gs#1, "Demo and measurements (M3)"):
// summaries, the acceptance thresholds and the PASS or MISS verdicts. Pure
// functions, no DOM, so the rules read in one place.

/** A frame interval over this many refresh periods is a missed frame. */
export const MISSED_FRAME_PERIODS = 1.5;

/** Acceptance thresholds, fixed before any measuring. P is the display's refresh period. */
export const THRESHOLDS = {
  /** Zoom and pan: p95 frame interval at most this many P. */
  smoothP95Periods: 1.25,
  /** Zoom and pan: missed frames under this percentage. */
  smoothMissedPct: 1,
  /** Drag: p95 drag response at most this many milliseconds. */
  dragResponseP95Ms: 50,
  /** Drag: missed frames under this percentage. */
  dragMissedPct: 5,
  /** Zoom anchor: the instant under the cursor moves by under this many pixels per step. */
  anchorDriftPx: 1,
} as const;

export interface Summary {
  count: number;
  p50: number;
  p95: number;
}

export interface FrameStats extends Summary {
  /** Intervals over 1.5 P. */
  missed: number;
  missedPct: number;
}

export type Verdict = "PASS" | "MISS";

export interface Check {
  name: string;
  value: number;
  /** The check passes when `value` is under the limit (at or under, for a p95 limit). */
  limit: number;
  unit: string;
  verdict: Verdict;
}

/** Nearest-rank percentile of an ascending list; NaN for an empty list. */
export function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) return Number.NaN;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length, Math.max(1, rank)) - 1] ?? Number.NaN;
}

export function summarize(values: readonly number[]): Summary {
  const sorted = [...values].sort((a, b) => a - b);
  return { count: sorted.length, p50: percentile(sorted, 50), p95: percentile(sorted, 95) };
}

/** p50, p95 and missed frames of consecutive requestAnimationFrame intervals. */
export function frameStats(intervals: readonly number[], periodMs: number): FrameStats {
  const missed = intervals.filter((v) => v > MISSED_FRAME_PERIODS * periodMs).length;
  return {
    ...summarize(intervals),
    missed,
    missedPct: intervals.length === 0 ? Number.NaN : (100 * missed) / intervals.length,
  };
}

function check(name: string, value: number, limit: number, unit: string, inclusive: boolean): Check {
  const ok = inclusive ? value <= limit : value < limit;
  return { name, value, limit, unit, verdict: ok ? "PASS" : "MISS" };
}

/** The verdicts against the thresholds. A value that is not a number is a MISS. */
export function judge(
  periodMs: number,
  zoom: FrameStats,
  pan: FrameStats,
  drag: FrameStats,
  dragResponse: Summary,
  maxAnchorDriftPx: number,
): Check[] {
  const p95Limit = THRESHOLDS.smoothP95Periods * periodMs;
  return [
    check("zoom: p95 frame interval <= 1.25 P", zoom.p95, p95Limit, "ms", true),
    check("zoom: missed frames < 1%", zoom.missedPct, THRESHOLDS.smoothMissedPct, "%", false),
    check("pan: p95 frame interval <= 1.25 P", pan.p95, p95Limit, "ms", true),
    check("pan: missed frames < 1%", pan.missedPct, THRESHOLDS.smoothMissedPct, "%", false),
    check("drag: p95 drag response <= 50 ms", dragResponse.p95, THRESHOLDS.dragResponseP95Ms, "ms", true),
    check("drag: missed frames < 5%", drag.missedPct, THRESHOLDS.dragMissedPct, "%", false),
    check("zoom anchor: drift per step < 1 px", maxAnchorDriftPx, THRESHOLDS.anchorDriftPx, "px", false),
  ];
}

/** A short-window view of the frame intervals for the live overlay. */
export class Rolling {
  private readonly values: number[] = [];
  private cursor = 0;
  constructor(private readonly size: number) {}

  push(v: number): void {
    if (this.values.length < this.size) this.values.push(v);
    else this.values[this.cursor] = v;
    this.cursor = (this.cursor + 1) % this.size;
  }

  get window(): readonly number[] {
    return this.values;
  }
}
