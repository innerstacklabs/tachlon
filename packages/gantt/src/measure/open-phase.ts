// The open phase: the frames after a file opens (gs#309). Part of the harness in `../measure.ts`.
//
// Each repeat re-opens the loaded schedule through `App.open`: the script writes the session out
// with `App.save` once, reads that document back with `LoadedSchedule.parse_document` (the object
// `App.open` consumes cannot be reused) and opens it. The open marks the bars of the rows in the
// window to draw in over 360 ms, so the 400 ms window (the edit phase's, plus its 40 ms margin)
// holds the whole draw-in. The read and the parse happen before the window starts; the window
// starts at the `App.open` call.
import { LoadedSchedule } from "../wasm/app.js";
import type { MeasureConfig } from "./config.js";
import { SLIDE_WINDOW_MS } from "./edit-phase.js";
import { PhaseRec, poolRecs } from "./phases.js";
import { SelectSteps } from "./select-phase.js";
import type { OpenResult } from "./result.js";

/** Frames let through after the open's window before the next repeat. */
const SETTLE_FRAMES = 3;

/** The open steps; `Metrics` runs `openRun` last (it replaces the schedule) and reads the result. */
export abstract class OpenSteps extends SelectSteps {
  private openRecs: PhaseRec[] = [];
  /** The document of a `?schedule` file; empty on the stress fixture. */
  private openDocument = "";
  private openTasks = 0;

  private *openOnce(record: boolean): Generator<void, PhaseRec> {
    const { app } = this.host;
    const rec = new PhaseRec("open");
    const schedule = this.openDocument === "" ? LoadedSchedule.stress(this.openTasks) : LoadedSchedule.parse_document(this.openDocument);
    yield;
    this.startPhase(record ? rec : null);
    app.open(schedule);
    for (const end = performance.now() + SLIDE_WINDOW_MS; performance.now() < end; ) yield;
    this.endPhase();
    for (let i = 0; i < SETTLE_FRAMES; i++) yield;
    return rec;
  }

  /** One unrecorded open if the run has a warm-up, then `editRepeats` recorded ones. */
  protected *openRun(config: MeasureConfig): Generator<void> {
    this.openTasks = this.host.status().n;
    if (config.scheduleSha256 !== undefined) this.openDocument = this.host.app.save("open-phase");
    yield;
    for (let r = config.warmup > 0 ? -1 : 0; r < config.editRepeats; r++) {
      this.progress = `open ${r >= 0 ? `${r + 1} of ${config.editRepeats}` : "warm-up"}`;
      const rec = yield* this.openOnce(r >= 0);
      if (r >= 0) this.openRecs.push(rec);
    }
  }

  protected openResult(period: number): OpenResult | null {
    return this.openRecs.length === 0 ? null : poolRecs(this.openRecs).result(period);
  }
}
