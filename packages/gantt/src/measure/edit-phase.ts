// The edit phase: a scripted duration edit on the stress fixture, and the frames
// after it (gs#419, story M of gs#309). Part of the harness in `../measure.ts`.
//
// Each repeat sets the duration of a task whose cone is wide through the public
// `App.set_duration` (to 200 days) and records two windows of frames: `slide` (0 to 400 ms
// after the edit: the 360 ms slide of the bars plus a 40 ms margin) and `afterEdit` (400 to
// 600 ms). It counts the rows whose start moved, reads the slide's copy time, undoes the edit,
// waits for the undo's slide to end and records 600 ms with no edit. The script chooses the
// task by trying the same edit on a few candidates; `moved` comes from reading
// every row's start before and after, so a phase that skipped the call reads 0. When the
// page reports no slide, both windows still record and `copyMs` reads null.
import { summarize } from "../metrics.js";
import { PhaseRec, ScriptSteps, poolRecs } from "./phases.js";
import type { MeasureConfig } from "./config.js";
import type { TableRow } from "../api.js";
import { slideOf, waitSlideIdle } from "./slide.js";
import type { EditResult } from "./result.js";

/** The slide window, ms after the edit: the slide's 360 ms plus a 40 ms margin. */
export const SLIDE_WINDOW_MS = 400;
/** The end of the `afterEdit` window, ms after the edit. */
export const AFTER_EDIT_END_MS = 600;
/** Length of the no-edit window, ms. */
export const QUIET_MS = 600;
/** The duration the edit sets, in the page's own duration text: longer than the fixture's tasks. */
const EDIT_TEXT = "200d";
/** Leaf tasks the script tries. */
const MAX_CANDIDATES = 32;
/** Frames let through after the undo before the no-edit window starts. */
export const SETTLE_FRAMES = 3;

interface EditRec {
  slide: PhaseRec;
  edit: PhaseRec;
  quiet: PhaseRec;
  moved: number;
  setMs: number;
  /** `status.slide.copyMs` after the edit; null when the page reports none. */
  copyMs: number | null;
}

/** The edit phase's steps; `Metrics` runs `editRun` after its repeats and reads `editResult`. */
export abstract class EditSteps extends ScriptSteps {
  /** What the script is doing, for the overlay. */
  abstract progress: string;

  private edits: EditRec[] = [];
  private chosen: { task: number; row: number; candidates: EditResult["candidates"] } | null = null;

  protected starts(): Float64Array {
    const { app } = this.host;
    const rows = this.host.status().n;
    const out = new Float64Array(rows);
    for (let r = 0; r < rows; r++) out[r] = app.row_start(r) ?? -1;
    return out;
  }

  /** Rows whose start differs from `before`. */
  protected movedFrom(before: Float64Array): number {
    const now = this.starts();
    let moved = 0;
    for (let r = 0; r < before.length; r++) if (now[r] !== before[r]) moved++;
    return moved;
  }

  /**
   * Leaf tasks in order of start, spread evenly from the earliest, from the table's own rows
   * (`App.table_rows`); a task that starts early has a long chain behind it.
   */
  private leafCandidates(): { task: number; row: number }[] {
    const { app, status } = this.host;
    const rows = JSON.parse(app.table_rows(0, status().n)) as TableRow[];
    const leaves: { task: number; row: number }[] = [];
    rows.forEach((t, row) => {
      if (t[5] === 0) leaves.push({ task: t[0], row });
    });
    leaves.sort((a, b) => (app.row_start(a.row) ?? 0) - (app.row_start(b.row) ?? 0));
    const step = Math.max(1, Math.floor(leaves.length / MAX_CANDIDATES));
    return leaves.filter((_, i) => i % step === 0).slice(0, MAX_CANDIDATES);
  }

  /** Try the edit on each candidate and keep the one that moves the most rows; every trial is undone. */
  private choose(before: Float64Array): NonNullable<EditSteps["chosen"]> {
    const { app } = this.host;
    const candidates: EditResult["candidates"] = [];
    for (const { task, row } of this.leafCandidates()) {
      app.set_duration(task, EDIT_TEXT);
      const moved = this.movedFrom(before);
      app.undo();
      candidates.push({ task, row, moved });
    }
    const best = candidates.reduce<(typeof candidates)[number] | null>((a, c) => (a === null || c.moved > a.moved ? c : a), null);
    if (best === null || best.moved === 0) throw new Error("no candidate task moves any row when its duration grows");
    if (this.movedFrom(before) !== 0) throw new Error("undo did not put the schedule back after the trial edits");
    return { task: best.task, row: best.row, candidates };
  }

  /** One recorded edit, or a warm-up one with `record` false. */
  private *editOnce(record: boolean, before: Float64Array): Generator<void, EditRec> {
    const { app } = this.host;
    const { task } = this.chosen!;
    const rec: EditRec = {
      slide: new PhaseRec("slide"),
      edit: new PhaseRec("edit"),
      quiet: new PhaseRec("quiet"),
      moved: 0,
      setMs: 0,
      copyMs: null,
    };
    const t0 = performance.now();
    app.set_duration(task, EDIT_TEXT);
    const t1 = performance.now();
    rec.setMs = t1 - t0;
    this.startPhase(record ? rec.slide : null);
    for (const end = t1 + SLIDE_WINDOW_MS; performance.now() < end; ) yield;
    this.endPhase();
    this.startPhase(record ? rec.edit : null);
    for (const end = t1 + AFTER_EDIT_END_MS; performance.now() < end; ) yield;
    this.endPhase();
    rec.moved = this.movedFrom(before);
    rec.copyMs = slideOf(this.host.status())?.copyMs ?? null;
    // An edit that moved nothing is reported as 0 moved (the spec fails on it), not hidden by an error here.
    if (!app.undo() && rec.moved > 0) throw new Error("rows moved but there was no edit to undo");
    if (this.movedFrom(before) !== 0) throw new Error("undo did not put the schedule back");
    for (let i = 0; i < SETTLE_FRAMES; i++) yield;
    // The undo slides the bars back; the quiet window starts after that, or it would record the slide.
    yield* waitSlideIdle(() => slideOf(this.host.status()));
    this.startPhase(record ? rec.quiet : null);
    for (const end = performance.now() + QUIET_MS; performance.now() < end; ) yield;
    this.endPhase();
    return rec;
  }

  /** The whole phase: choose the task, one unrecorded edit if the run has a warm-up, then `editRepeats` recorded ones. */
  protected *editRun(config: MeasureConfig): Generator<void> {
    const before = this.starts();
    this.chosen = this.choose(before);
    yield;
    for (let r = config.warmup > 0 ? -1 : 0; r < config.editRepeats; r++) {
      const record = r >= 0;
      this.progress = `edit ${record ? `${r + 1} of ${config.editRepeats}` : "warm-up"}`;
      const rec = yield* this.editOnce(record, before);
      if (record) this.edits.push(rec);
    }
  }

  /** The result, or null when no edit was recorded. */
  protected editResult(period: number): EditResult | null {
    const chosen = this.chosen;
    if (chosen === null || this.edits.length === 0) return null;
    const moved = this.edits.map((e) => e.moved);
    const copies = this.edits.flatMap((e) => (e.copyMs === null ? [] : [e.copyMs]));
    return {
      task: chosen.task,
      row: chosen.row,
      text: EDIT_TEXT,
      moved: Math.min(...moved),
      movedPerRepeat: moved,
      candidates: chosen.candidates,
      setMs: summarize(this.edits.map((e) => e.setMs)),
      copyMs: copies.length === 0 ? null : summarize(copies),
      slide: poolRecs(this.edits.map((e) => e.slide)).result(period),
      afterEdit: poolRecs(this.edits.map((e) => e.edit)).result(period),
      noEdit: poolRecs(this.edits.map((e) => e.quiet)).result(period),
    };
  }
}
