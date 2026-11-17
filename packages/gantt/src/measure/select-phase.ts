// The select and Fit phase: the frames after a task is selected and after Fit, on the stress
// fixture (gs#309). Part of the harness in `../measure.ts`.
//
// The script chooses a task with a long driving chain by trial (it selects each of a spread of
// leaf tasks through the public `App.select_task` and reads the length of the chain from
// `status.selected`). Each repeat then selects the pick task and records 400 ms, clears the
// selection, zooms in and calls `App.fit_project`, and records another 400 ms. The 400 ms is the
// edit phase's slide window, so the numbers sit beside it.
import type { TableRow } from "../api.js";
import type { MeasureConfig } from "./config.js";
import { LinkEditSteps } from "./link-edit-phase.js";
import { SLIDE_WINDOW_MS } from "./edit-phase.js";
import { PhaseRec, poolRecs } from "./phases.js";
import type { FitResult, SelectResult } from "./result.js";

/** Leaf tasks the script tries. */
const MAX_CANDIDATES = 32;
/** Wheel step per frame while zooming in before Fit, as in the zoom sweep. */
const ZOOM_IN_STEP = 60;
/** The scale the zoom reaches before Fit, px per minute: a day across 144 px, the pan view's. */
const ZOOMED_PX_PER_MINUTE = 0.1;
/** Frames let through between steps. */
const SETTLE_FRAMES = 3;

interface Rec {
  select: PhaseRec;
  fit: PhaseRec;
  /** Scale before and after Fit, px per minute. */
  before: number;
  after: number;
}

/** The select and Fit steps; `Metrics` runs `selectRun` after the link-edit phase and reads the results. */
export abstract class SelectSteps extends LinkEditSteps {
  private selectRecs: Rec[] = [];
  private pick: { task: number; row: number; back: number; candidates: SelectResult["candidates"] } | null = null;

  /** Select each candidate, keep the one with the longest driving chain behind it, and clear the selection. */
  private choosePick(): NonNullable<SelectSteps["pick"]> {
    const { app, status } = this.host;
    const rows = JSON.parse(app.table_rows(0, status().n)) as TableRow[];
    const leaves: { task: number; row: number }[] = [];
    rows.forEach((t, row) => {
      if (t[5] === 0) leaves.push({ task: t[0], row });
    });
    const step = Math.max(1, Math.floor(leaves.length / MAX_CANDIDATES));
    const candidates: SelectResult["candidates"] = [];
    for (const { task, row } of leaves.filter((_, i) => i % step === 0).slice(0, MAX_CANDIDATES)) {
      const back = app.select_task(task) ? (status().selected?.backCount ?? 0) : 0;
      app.clear_selection();
      candidates.push({ task, row, back });
    }
    const best = candidates.reduce<(typeof candidates)[number] | null>((a, c) => (a === null || c.back > a.back ? c : a), null);
    if (best === null || best.back === 0) throw new Error("no candidate task has a driving chain to select");
    return { ...best, candidates };
  }

  private *selectWindow(rec: PhaseRec | null): Generator<void> {
    this.startPhase(rec);
    for (const end = performance.now() + SLIDE_WINDOW_MS; performance.now() < end; ) yield;
    this.endPhase();
  }

  private *selectOnce(record: boolean): Generator<void, Rec> {
    const { app, status, canvas } = this.host;
    const { task } = this.pick!;
    const rec: Rec = { select: new PhaseRec("select"), fit: new PhaseRec("fit"), before: 0, after: 0 };
    if (!app.select_task(task)) throw new Error(`select_task refused task ${task}`);
    yield* this.selectWindow(record ? rec.select : null);
    app.clear_selection();
    for (let i = 0; i < SETTLE_FRAMES; i++) yield;
    // Zoom in on the middle of the canvas so that Fit has a view to leave.
    yield* this.zoom(canvas.getBoundingClientRect().width / 2, -ZOOM_IN_STEP, (p) => p >= ZOOMED_PX_PER_MINUTE, false);
    for (let i = 0; i < SETTLE_FRAMES; i++) yield;
    rec.before = status().pxPerMinute;
    if (!app.fit_project()) throw new Error("fit_project found no live task");
    yield* this.selectWindow(record ? rec.fit : null);
    rec.after = status().pxPerMinute;
    if (rec.after === rec.before) throw new Error("Fit did not change the scale");
    return rec;
  }

  /** The whole phase: choosePick the task, one unrecorded repeat if the run has a warm-up, then `editRepeats` recorded ones. */
  protected *selectRun(config: MeasureConfig): Generator<void> {
    this.pick = this.choosePick();
    yield;
    for (let r = config.warmup > 0 ? -1 : 0; r < config.editRepeats; r++) {
      this.progress = `select and Fit ${r >= 0 ? `${r + 1} of ${config.editRepeats}` : "warm-up"}`;
      const rec = yield* this.selectOnce(r >= 0);
      if (r >= 0) this.selectRecs.push(rec);
    }
  }

  protected selectResult(period: number): SelectResult | null {
    const c = this.pick;
    if (c === null || this.selectRecs.length === 0) return null;
    return { task: c.task, row: c.row, back: c.back, candidates: c.candidates, ...poolRecs(this.selectRecs.map((r) => r.select)).result(period) };
  }

  protected fitResult(period: number): FitResult | null {
    if (this.selectRecs.length === 0) return null;
    return {
      pxPerMinuteBefore: this.selectRecs.map((r) => r.before),
      pxPerMinuteAfter: this.selectRecs.map((r) => r.after),
      ...poolRecs(this.selectRecs.map((r) => r.fit)).result(period),
    };
  }
}
