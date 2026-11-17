// Frame intervals and what the benchmark reports from them: p50, p95, long gaps and dropped frames.
//
// The driver records the gap between `requestAnimationFrame` callbacks while a phase runs, the
// same recorder on every side, and `summarise` turns the gaps into the numbers the table prints.
// A long gap is one over LONG_GAP_PERIODS display periods (1.5 at 60 Hz: 25 ms); the frames it
// dropped are the display periods it spans, rounded, less the one frame it is.

/** A gap over this many display periods is a long gap. */
export const LONG_GAP_PERIODS = 1.5;

export interface FrameStats {
  /** Gaps recorded, one per frame after the first. */
  frames: number;
  /** Median gap, ms. */
  p50: number;
  /** 95th percentile gap, ms. */
  p95: number;
  /** Gaps over LONG_GAP_PERIODS display periods. */
  longGaps: number;
  /** Display frames with no new frame: for each gap, its periods rounded, less one. */
  droppedFrames: number;
  /** The worst gap, ms. */
  max: number;
}

/** The nearest-rank percentile `p` (0 to 100) of `values`; NaN for an empty list. */
export function percentile(values: number[], p: number): number {
  if (values.length === 0) return Number.NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1];
}

/** Round to 0.01 ms, so a result file does not carry timer noise past what it can mean. */
const ms = (v: number) => Math.round(v * 100) / 100;

/** The numbers for one phase's gaps at a display of `refreshHz`. */
export function summarise(gaps: number[], refreshHz: number): FrameStats {
  const period = 1000 / refreshHz;
  return {
    frames: gaps.length,
    p50: ms(percentile(gaps, 50)),
    p95: ms(percentile(gaps, 95)),
    longGaps: gaps.filter((g) => g > LONG_GAP_PERIODS * period).length,
    droppedFrames: gaps.reduce((n, g) => n + Math.max(0, Math.round(g / period) - 1), 0),
    max: ms(gaps.length === 0 ? Number.NaN : Math.max(...gaps)),
  };
}

// The in-page recorder. These run in the page (Playwright serialises them), so they close over
// nothing and keep their state on one window property that no side uses.

/** Start recording frame gaps in the page. */
export function startRecorder(): void {
  const w = window as unknown as Record<string, { on: boolean; last: number; gaps: number[] }>;
  const rec = { on: true, last: -1, gaps: [] as number[] };
  w.__benchFrames = rec;
  const tick = (now: number) => {
    if (!rec.on) return;
    if (rec.last >= 0) rec.gaps.push(now - rec.last);
    rec.last = now;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

/** Stop recording and return the gaps, ms. */
export function stopRecorder(): number[] {
  const w = window as unknown as Record<string, { on: boolean; gaps: number[] } | undefined>;
  const rec = w.__benchFrames;
  if (rec === undefined) return [];
  rec.on = false;
  w.__benchFrames = undefined;
  return rec.gaps;
}

/** Resolve on the page's next frame: one input step per frame. */
export function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}
