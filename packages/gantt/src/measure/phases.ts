// The frames of one phase, and the script steps that drive them (plan gs#1,
// "Demo and measurements (M3)"). Part of the harness in `../measure.ts`.
import { frameStats, summarize } from "../metrics.js";
import type { Host, HostInit } from "./config.js";
import type { PhaseResult } from "./result.js";

/** Frames recorded for one phase of one repeat. */
export class PhaseRec {
  intervals: number[] = [];
  encode: number[] = [];
  responses: number[] = [];
  recompute: number[] = [];
  /** The `requestAnimationFrame` timestamps of the phase's first frame and of the frame after its last. */
  startAt = Number.NaN;
  endAt = Number.NaN;
  constructor(readonly name: "zoom" | "pan" | "drag" | "slide" | "edit" | "quiet" | "select" | "fit" | "open") {}
  result(period: number): PhaseResult {
    return {
      ...frameStats(this.intervals, period),
      encodeMs: summarize(this.encode),
      totalMs: this.intervals.reduce((a, v) => a + v, 0),
      spanMs: this.endAt - this.startAt,
    };
  }
}

/** The recorded repeats of one phase pooled into one record: the intervals and CPU times joined, the spans added. */
export function poolRecs(recs: readonly PhaseRec[]): PhaseRec {
  const all = new PhaseRec(recs[0]!.name);
  all.startAt = 0;
  all.endAt = 0;
  for (const p of recs) {
    all.endAt += p.endAt - p.startAt;
    all.intervals.push(...p.intervals);
    all.encode.push(...p.encode);
    all.responses.push(...p.responses);
    all.recompute.push(...p.recompute);
  }
  return all;
}

export interface RepeatRec {
  zoom: PhaseRec;
  pan: PhaseRec;
  drag: PhaseRec;
}

const MAX_ZOOM_STEPS = 400;

/** The scripted input a run is made of: zoom, pan and pointer moves against the page's own input functions. */
export abstract class ScriptSteps {
  protected phase: PhaseRec | null = null;
  /** The requestAnimationFrame timestamp of the callback in progress. */
  protected frameStart = 0;
  protected drift = { steps: 0, max: 0 };

  protected readonly host: Host;

  constructor(init: HostInit) {
    this.host = init as Host;
  }

  /**
   * The scripted pointer moves to (x, y). There is no real event, so its
   * timestamp is `performance.now()` as it is issued, the clock of a real
   * event's `timeStamp`. This frame applies the move; the response ends at the
   * next frame's start, after the recompute, the table update and the draw.
   */
  protected stampedMove(x: number, y: number): void {
    this.host.input.move(x, y, performance.now());
  }

  protected startPhase(rec: PhaseRec | null): void {
    this.phase = rec;
    if (rec) rec.startAt = this.frameStart;
  }

  /** Called in the callback after the phase's last frame; that frame's interval is already recorded. */
  protected endPhase(): void {
    if (this.phase) this.phase.endAt = this.frameStart;
    this.phase = null;
  }

  /** One ctrl+wheel step at x; the anchor error in px. */
  protected zoomStep(x: number, dy: number): number {
    const { app, input } = this.host;
    const t = app.time_at(x);
    input.wheel(x, 0, 0, dy, true);
    return Math.abs(app.x_of(t) - x);
  }

  /** Zoom one step per frame until the scale stops moving or `until` holds. */
  protected *zoom(x: number, dy: number, until: (pxPerMinute: number) => boolean, count: boolean): Generator<void> {
    for (let i = 0; i < MAX_ZOOM_STEPS; i++) {
      const before = this.host.status().pxPerMinute;
      const d = this.zoomStep(x, dy);
      if (count) {
        this.drift.steps++;
        this.drift.max = Math.max(this.drift.max, d);
      }
      const after = this.host.status().pxPerMinute;
      yield;
      if (after === before || until(after)) return;
    }
    throw new Error(`zoom did not settle in ${MAX_ZOOM_STEPS} steps`);
  }

  /** First row whose task starts at or after `t`. */
  protected rowAt(t: number): number {
    const { app, status } = this.host;
    let lo = 0;
    let hi = status().n;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if ((app.row_start(mid) ?? Number.POSITIVE_INFINITY) < t) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /** Pan so that instant `t` is at x (px from the canvas's left edge). */
  protected panTo(t: number, x: number): void {
    const { app, input } = this.host;
    for (let i = 0; i < 3; i++) {
      const dx = app.x_of(t) - x;
      if (Math.abs(dx) < 0.5) return;
      input.wheel(x, 0, dx, 0, false);
    }
  }
}
