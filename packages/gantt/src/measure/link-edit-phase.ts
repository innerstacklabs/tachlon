// The link-edit phase: a scripted link added, removed, undone and redone on the stress fixture, and the
// frames after each (gs#725; since gs#724 a link edit starts the slide, the outline and the note like a duration
// edit). Part of the harness in `../measure.ts`; it runs after the duration edit of `edit-phase.ts`.
//
// There is no wasm method that adds a link, so the script takes the pointer path a person takes: an Alt press on
// a leaf bar, a move onto another leaf bar, a release. It chooses the pair by trying the same drag, undone, on
// pairs of bars on screen: a late bar as the predecessor of an early one, whose cone is wide. A pair the engine
// refuses (a loop, a duplicate) leaves the revision as it was and is skipped. Each repeat then adds the link,
// records the `slide` (0 to 400 ms after) and `afterEdit` (400 to 600 ms) windows and `status.slide.copyMs`,
// removes it with `App.remove_selected_link` (two more windows), undoes the removal and redoes it (a third
// pair: the 400 ms after each, and the 200 ms after the redo), waits for the slide to end and records 600 ms
// with no edit. `moved` comes from reading every row's start before and after.
import { summarize } from "../metrics.js";
import { PhaseRec, poolRecs } from "./phases.js";
import type { MeasureConfig } from "./config.js";
import type { TableRow } from "../api.js";
import { slideOf, waitSlideIdle } from "./slide.js";
import { AFTER_EDIT_END_MS, EditSteps, QUIET_MS, SETTLE_FRAMES, SLIDE_WINDOW_MS } from "./edit-phase.js";
import type { LinkEditResult } from "./result.js";

/** Pairs the script tries: the latest-starting bars on screen as predecessors of the earliest-starting ones. */
const MAX_PREDS = 8;
const MAX_SUCCS = 8;
/** Pointer moves between the press and the release, so the drag passes the threshold and the target is hovered. */
const DRAG_STEPS = 3;

interface Bar {
  task: number;
  row: number;
  x: number;
  y: number;
}

interface Pair {
  pred: Bar;
  succ: Bar;
}

interface LinkRec {
  slide: PhaseRec;
  edit: PhaseRec;
  removeSlide: PhaseRec;
  removeEdit: PhaseRec;
  undoSlide: PhaseRec;
  redoSlide: PhaseRec;
  redoEdit: PhaseRec;
  quiet: PhaseRec;
  moved: number;
  setMs: number;
  copyMs: number | null;
}

/** The link-edit phase's steps; `Metrics` runs `linkEditRun` after `editRun` and reads `linkEditResult`. */
export abstract class LinkEditSteps extends EditSteps {
  private links: LinkRec[] = [];
  private pair: Pair | null = null;
  private tried: LinkEditResult["candidates"] = [];
  private refused = 0;

  /** Leaf tasks whose bar is on screen, earliest start first, each with a point inside its bar (canvas CSS px). */
  private visibleBars(): Bar[] {
    const { app, status, canvas } = this.host;
    const s = status();
    const box = canvas.getBoundingClientRect();
    const rows = JSON.parse(app.table_rows(0, s.n)) as TableRow[];
    const bars: Bar[] = [];
    rows.forEach((t, row) => {
      if (t[5] !== 0) return;
      const es = app.row_start(row);
      if (es === undefined) return;
      const y = s.axisHeight + row * s.rowHeight - s.scrollY + s.rowHeight / 2;
      const x = app.x_of(es) + 1; // a bar is at least 2 px wide
      if (y < s.axisHeight || y > box.height || x < 0 || x > box.width - 1) return;
      bars.push({ task: t[0], row, x, y });
    });
    return bars.sort((a, b) => (app.row_start(a.row) ?? 0) - (app.row_start(b.row) ?? 0));
  }

  /** Alt-press on `pred`'s bar and move onto `succ`'s, leaving the button down. */
  private dragTo({ pred, succ }: Pair): void {
    const { input } = this.host;
    input.move(pred.x, pred.y);
    input.down(pred.x, pred.y, true);
    for (let i = 1; i <= DRAG_STEPS; i++) {
      const k = i / DRAG_STEPS;
      input.move(pred.x + (succ.x - pred.x) * k, pred.y + (succ.y - pred.y) * k);
    }
  }

  /** Alt-drag from `pred`'s bar onto `succ`'s and release; true when the schedule changed (the link was added). */
  private dropLink(pair: Pair): boolean {
    const { input, status } = this.host;
    const revision = status().revision;
    this.dragTo(pair);
    input.up();
    return status().revision !== revision;
  }

  /** Try the drop on each pair and keep the one that moves the most rows; every trial is undone. */
  private choosePair(before: Float64Array): Pair {
    const bars = this.visibleBars();
    if (bars.length < 2) throw new Error(`only ${bars.length} leaf bars are on screen: no pair to link`);
    const succs = bars.slice(0, MAX_SUCCS);
    const preds = bars.slice(-MAX_PREDS).reverse();
    let best: (Pair & { moved: number }) | null = null;
    for (const succ of succs) {
      for (const pred of preds) {
        if (pred.task === succ.task) continue;
        const pair = { pred, succ };
        let moved = 0;
        if (this.dropLink(pair)) {
          moved = this.movedFrom(before);
          this.host.app.undo();
        } else {
          this.refused++;
        }
        this.tried.push({ pred: pred.task, task: succ.task, predRow: pred.row, row: succ.row, moved });
        if (best === null || moved > best.moved) best = { ...pair, moved };
      }
    }
    if (best === null || best.moved === 0) throw new Error("no pair of bars on screen moves any row when it is linked");
    if (this.movedFrom(before) !== 0) throw new Error("undo did not put the schedule back after the trial links");
    return { pred: best.pred, succ: best.succ };
  }

  /** The frames in the `slide` window, then in the rest of the `afterEdit` one, both counted from `t1`. */
  private *windows(slide: PhaseRec | null, edit: PhaseRec | null, t1: number): Generator<void> {
    this.startPhase(slide);
    for (const end = t1 + SLIDE_WINDOW_MS; performance.now() < end; ) yield;
    this.endPhase();
    this.startPhase(edit);
    for (const end = t1 + AFTER_EDIT_END_MS; performance.now() < end; ) yield;
    this.endPhase();
  }

  /** The frames in the 400 ms after `t1`, as one window. */
  private *slideWindow(rec: PhaseRec | null, t1: number): Generator<void> {
    this.startPhase(rec);
    for (const end = t1 + SLIDE_WINDOW_MS; performance.now() < end; ) yield;
    this.endPhase();
  }

  /** One recorded repeat, or a warm-up one with `record` false. */
  private *linkOnce(record: boolean, before: Float64Array): Generator<void, LinkRec> {
    const { app } = this.host;
    const pair = this.pair!;
    const rec = (name: PhaseRec["name"]): PhaseRec => new PhaseRec(name);
    const r: LinkRec = {
      slide: rec("slide"),
      edit: rec("edit"),
      removeSlide: rec("slide"),
      removeEdit: rec("edit"),
      undoSlide: rec("slide"),
      redoSlide: rec("slide"),
      redoEdit: rec("edit"),
      quiet: rec("quiet"),
      moved: 0,
      setMs: 0,
      copyMs: null,
    };
    const pick = (p: PhaseRec): PhaseRec | null => (record ? p : null);
    const { input, status } = this.host;

    // The add: the press and the moves are not timed; the release is the edit.
    this.dragTo(pair);
    const revision = status().revision;
    const t0 = performance.now();
    input.up();
    const t1 = performance.now();
    r.setMs = t1 - t0;
    if (status().revision === revision) throw new Error("the chosen pair was not linked");
    yield* this.windows(pick(r.slide), pick(r.edit), t1);
    r.moved = this.movedFrom(before);
    r.copyMs = slideOf(status())?.copyMs ?? null;

    // The removal of the link the add selected.
    if (app.selected_link() === undefined) throw new Error("the added link is not selected");
    const t2 = performance.now();
    app.remove_selected_link();
    yield* this.windows(pick(r.removeSlide), pick(r.removeEdit), t2);
    if (this.movedFrom(before) !== 0) throw new Error("removing the link did not put the schedule back");

    // Undo puts the link back; redo removes it again.
    const t3 = performance.now();
    if (!app.undo()) throw new Error("no removal to undo");
    yield* this.slideWindow(pick(r.undoSlide), t3);
    const t4 = performance.now();
    if (!app.redo()) throw new Error("no removal to redo");
    yield* this.windows(pick(r.redoSlide), pick(r.redoEdit), t4);
    if (this.movedFrom(before) !== 0) throw new Error("the redo did not put the schedule back");

    for (let i = 0; i < SETTLE_FRAMES; i++) yield;
    yield* waitSlideIdle(() => slideOf(status()));
    this.startPhase(pick(r.quiet));
    for (const end = performance.now() + QUIET_MS; performance.now() < end; ) yield;
    this.endPhase();
    return r;
  }

  /** The whole phase: choose the pair, one unrecorded repeat if the run has a warm-up, then `editRepeats` recorded ones. */
  protected *linkEditRun(config: MeasureConfig): Generator<void> {
    const before = this.starts();
    this.pair = this.choosePair(before);
    yield;
    for (let r = config.warmup > 0 ? -1 : 0; r < config.editRepeats; r++) {
      const record = r >= 0;
      this.progress = `link edit ${record ? `${r + 1} of ${config.editRepeats}` : "warm-up"}`;
      const rec = yield* this.linkOnce(record, before);
      if (record) this.links.push(rec);
    }
  }

  /** The result, or null when no link edit was recorded. */
  protected linkEditResult(period: number): LinkEditResult | null {
    const pair = this.pair;
    if (pair === null || this.links.length === 0) return null;
    const moved = this.links.map((e) => e.moved);
    const copies = this.links.flatMap((e) => (e.copyMs === null ? [] : [e.copyMs]));
    const pool = (pick: (e: LinkRec) => PhaseRec[]) => poolRecs(this.links.flatMap(pick)).result(period);
    return {
      pred: pair.pred.task,
      predRow: pair.pred.row,
      task: pair.succ.task,
      row: pair.succ.row,
      moved: Math.min(...moved),
      movedPerRepeat: moved,
      candidates: this.tried,
      refused: this.refused,
      setMs: summarize(this.links.map((e) => e.setMs)),
      copyMs: copies.length === 0 ? null : summarize(copies),
      slide: pool((e) => [e.slide]),
      afterEdit: pool((e) => [e.edit]),
      remove: { slide: pool((e) => [e.removeSlide]), afterEdit: pool((e) => [e.removeEdit]) },
      undoRedo: { slide: pool((e) => [e.undoSlide, e.redoSlide]), afterEdit: pool((e) => [e.redoEdit]) },
      noEdit: pool((e) => [e.quiet]),
    };
  }
}
