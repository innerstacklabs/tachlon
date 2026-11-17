// What `Gantt.speed()` returns: the last open's phases and the last frames' numbers. It reads records the loop already keeps and calls no
// `App` method: `loop.ts` wraps every one, so a call from outside a tick wakes the frame loop, and a readout that woke it would keep the chart drawing.
import { frameStats } from "./metrics.js";
import type { OpenTimings } from "./open-schedule.js";

/** What `Gantt.speed()` returns. */
export interface Speed {
  /** The last timed open, by phase; null before one and after `load`. */
  open: { name: string; timings: OpenTimings } | null;
  /** The last 120 frames in ms and the refresh rate; null until 120 frames are drawn. */
  frames: { count: number; p50: number; p95: number; missed: number; refreshHz: number } | null;
}

/** A copy of the open facts and the frame window's numbers. */
export function speedOf(open: { name: string; timings: OpenTimings } | null, view: { window: readonly number[]; period: number | null }): Speed {
  const { window, period } = view;
  const { count, p50, p95, missed } = frameStats(window, period ?? 1);
  return {
    open: open === null ? null : { name: open.name, timings: { ...open.timings } },
    frames: period === null ? null : { count, p50, p95, missed, refreshHz: 1000 / period },
  };
}
