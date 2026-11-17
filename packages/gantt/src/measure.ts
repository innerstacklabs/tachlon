// The metrics overlay and the scripted run (plan gs#1, "Demo and measurements
// (M3)"). `Metrics` watches every frame for the live overlay; with `?measure=1`
// it also drives a script through the page's own input functions, records
// the numbers the plan names, judges them against the thresholds in
// `metrics.ts` and publishes one JSON result.
//
// The script is a generator: each `yield` waits for the next frame; a step is applied at the start of a rAF callback and drawn in it.
//
// This file keeps `Metrics` and is the harness's entry: the configuration, the result's shape, the
// browser facts and the script's steps live in `measure/`; what the page and the tests import is re-exported here.
import type { Status, TableRow } from "./api.js";
import { MISSED_FRAME_PERIODS, Rolling, frameStats, judge, percentile, summarize } from "./metrics.js";
import type { Environment, HostInit, MeasureConfig, MeasurementResult } from "./measure/config.js";
import { dragCandidates } from "./measure/critical.js";
import { browserVersion, webglRenderer, webgpuAdapter } from "./measure/env.js";
import { OpenSteps } from "./measure/open-phase.js";
import { PhaseRec, type RepeatRec, poolRecs } from "./measure/phases.js";
import type { MeasurementError } from "./measure/result.js";

export { measureConfig } from "./measure/config.js";
export type { Environment, Host, HostInit, Input, MeasureConfig, MeasurementResult } from "./measure/config.js";
export type { MeasurementError, PhaseResult, Repeat } from "./measure/result.js";

/** Zoom wheel step: factor exp(60 * 0.002) per frame, about 80 steps from 5 years to 2 hours. */
const ZOOM_STEP = 60;
/** Minutes in a day, as in the engine. */
const DAY = 1440;
/** Pixels the dragged pointer moves per frame. */
const DRAG_PX_PER_FRAME = 5;
/** Scale of the pan view: 0.1 px per minute puts a day at 144 px. */
const WORK_PX_PER_MINUTE = 0.1;
/** Display rates the estimated refresh rate is checked against, and how near (as a fraction) counts as the same. */
const STANDARD_REFRESH_HZ = [60, 75, 90, 120, 144, 165, 240], REFRESH_TOLERANCE = 0.04;
const DRAG_TRIALS = 16, IDLE_FRAMES_SKIPPED = 30, IDLE_FRAMES_USED = 90;

/**
 * The refresh period from idle intervals: their 10th percentile. A frame that
 * misses a refresh takes a whole number of periods more, so the slow ones sit
 * above the period; the median would take a loaded start-up for the period.
 */
function refreshPeriod(idle: readonly number[]): number {
  return percentile([...idle].sort((a, b) => a - b), 10);
}

export class Metrics extends OpenSteps {
  /** Called once with the result or the error. */
  onDone: (outcome: MeasurementResult | MeasurementError) => void = () => {};
  /** What the script is doing, for the overlay. */
  progress = "";
  private readonly frames = new Rolling(120);
  private readonly encodes = new Rolling(120);
  private seen = 0;
  private readonly calibration: number[] = [];
  private period: number | null = null;
  private periodSource: Environment["refreshSource"] = "10th percentile of 90 idle frames";
  /** Pointer events applied by the frame in progress, by `timeStamp`. */
  private applied: number[] = [];
  /** Moves applied by the last frame; each ends at the next frame's start. */
  private pending: { at: number; into: number[] }[] = [];
  private lastStamp = -1;
  /** The scripted run, null with none or once it has ended; the loop draws every frame while it is set. */ script: Generator<void, void> | null = null;
  private repeats: RepeatRec[] = [];
  private liveText: string[] = [];
  private liveAge = 0;
  private dragRow: number | null = null;

  constructor(
    init: HostInit,
    private readonly config: MeasureConfig | null,
  ) {
    super(init);
    if (config?.refreshHz) {
      this.period = 1000 / config.refreshHz;
      this.periodSource = "refreshHz option";
    }
    if (config) this.script = this.run(config);
  }

  /**
   * The start of a requestAnimationFrame callback, before the frame is drawn.
   * `interval` is the time since the previous callback: the duration of the
   * previous frame, so it belongs to the phase that frame ran in. It is
   * recorded before the script advances, which may end that phase.
   */
  begin(now: number, interval: number): void {
    this.frameStart = now;
    this.phase?.intervals.push(interval);
    for (const p of this.pending) p.into.push(now - p.at);
    this.pending = [];
    if (!this.script) return;
    try {
      if (this.script.next().done) {
        this.script = null;
        void this.finish().then(
          (result) => this.onDone(result),
          (err: unknown) => this.onDone({ error: String(err) }),
        );
      }
    } catch (err) {
      this.script = null;
      this.progress = `failed: ${String(err)}`;
      this.onDone({ error: String(err) });
    }
  }

  /** A pointer move was applied to the app. `at` is the event's `timeStamp`. */
  moved(at: number): void {
    this.applied.push(at);
  }

  /** The end of the callback: the frame is drawn and the table is updated. */
  end(interval: number, encodeMs: number, s: Status): void {
    this.seen++;
    if (this.seen > 2) {
      this.frames.push(interval);
      this.encodes.push(encodeMs);
    }
    if (this.period === null && this.seen > IDLE_FRAMES_SKIPPED) {
      this.calibration.push(interval);
      if (this.calibration.length >= IDLE_FRAMES_USED) this.period = refreshPeriod(this.calibration);
    }
    const rec = this.phase;
    if (rec) {
      rec.encode.push(encodeMs);
      // A new solution this frame means a recompute; its time is the last drag's.
      if (s.drag && s.stamp !== this.lastStamp) rec.recompute.push(s.drag.recomputeMs);
    }
    // The moves this frame applied are answered by the next frame's start.
    if (rec?.name === "drag") for (const at of this.applied) this.pending.push({ at, into: rec.responses });
    this.applied = [];
    this.lastStamp = s.stamp;
  }

  /** What `Gantt.speed()` reads: the last frame intervals and the refresh period (null until measured). No call into the app. */
  view = (): { window: readonly number[]; period: number | null } => ({ window: this.frames.window, period: this.period });

  /** Overlay lines: the rolling frame numbers, and the run's progress. */
  lines(): string[] {
    if (this.liveAge++ % 15 === 0) {
      const period = this.period;
      if (period === null) {
        this.liveText = [`frame     measuring the refresh period (${this.calibration.length} of ${IDLE_FRAMES_USED})`];
      } else {
        const f = frameStats(this.frames.window, period);
        const e = summarize(this.encodes.window);
        this.liveText = [
          `frame     p50 ${f.p50.toFixed(1)} · p95 ${f.p95.toFixed(1)} ms, missed ${f.missed} of ${f.count} (over ${(MISSED_FRAME_PERIODS * period).toFixed(1)} ms; refresh ${(1000 / period).toFixed(1)} Hz)`,
          `encode    p50 ${e.p50.toFixed(2)} · p95 ${e.p95.toFixed(2)} ms of CPU per frame`,
        ];
      }
    }
    return this.config ? [...this.liveText, `measure   ${this.progress}`] : this.liveText;
  }

  private *repeat(tMid: number, record: boolean): Generator<void, RepeatRec> {
    const { app, input, canvas, status } = this.host;
    const width = canvas.getBoundingClientRect().width;
    const cx = width / 2;
    const rec: RepeatRec = { zoom: new PhaseRec("zoom"), pan: new PhaseRec("pan"), drag: new PhaseRec("drag") };

    // Zoom sweep. Centre the project's middle with rows that start there under
    // the cursor, go out to five years, then record in to two hours and back.
    const s = status();
    app.set_scroll_y(Math.max(0, this.rowAt(tMid) - 10) * s.rowHeight);
    this.panTo(tMid, cx);
    this.progress = `${this.label(record)}: zoom out to the limit`;
    yield* this.zoom(cx, ZOOM_STEP, () => false, false);
    this.progress = `${this.label(record)}: zoom sweep`;
    this.startPhase(record ? rec.zoom : null);
    yield* this.zoom(cx, -ZOOM_STEP, () => false, record);
    yield* this.zoom(cx, ZOOM_STEP, () => false, record);
    this.endPhase();

    // Pan: press in the axis band (never on a bar) and sweep both axes.
    this.progress = `${this.label(record)}: pan`;
    yield* this.zoom(cx, -ZOOM_STEP, (p) => p >= WORK_PX_PER_MINUTE, false);
    const ay = status().axisHeight / 2;
    const amp = Math.min(350, width * 0.4);
    this.startPhase(record ? rec.pan : null);
    input.down(cx, ay, false);
    for (let k = 1, end = performance.now() + this.config!.panMs; performance.now() < end; k++) {
      input.move(cx + amp * Math.sin(k * 0.07), ay + 75 * (1 - Math.cos(k * 0.05)));
      yield;
    }
    input.up();
    this.endPhase();

    // Drag: put the head task (on a file, the trial's choice) near the left, press on its bar and sweep.
    this.progress = `${this.label(record)}: drag`;
    const sweep = Math.min(300, width * 0.6);
    const dragScale = sweep / (this.config!.dragDays * DAY);
    yield* this.zoom(cx, ZOOM_STEP, (p) => p <= dragScale, false);
    const head = app.test_head_row();
    if (head === undefined && this.config!.scheduleSha256 === undefined) throw new Error("the page is not on the stress fixture: no head task");
    const row = (this.dragRow ??= (this.config!.dragTarget === "critical" ? undefined : head) ?? (yield* this.chooseDragRow(width, sweep)));
    const { px, py } = yield* this.aim(row, width);
    input.move(px, py);
    if (!status().hover) throw new Error(`no bar under the pointer at the head task (row ${row}, ${px.toFixed(0)}, ${py.toFixed(0)})`);
    this.startPhase(record ? rec.drag : null);
    input.down(px, py, false);
    const half = sweep / DRAG_PX_PER_FRAME;
    for (let k = 1, end = performance.now() + this.config!.dragMs; performance.now() < end; k++) {
      const tri = k % (2 * half);
      const offset = (tri <= half ? tri : 2 * half - tri) * DRAG_PX_PER_FRAME;
      this.stampedMove(px + 5 + offset, py);
      yield;
    }
    this.endPhase();
    const dragged = status().drag;
    if (!dragged?.active) throw new Error("the drag did not start");
    input.cancel(); // puts the head task back, so every repeat starts from the same schedule
    yield;
    yield;
    return rec;
  }

  /** Scroll `row` into view with its start near the left, and return where to press on its bar. */
  private *aim(row: number, width: number): Generator<void, { px: number; py: number }> {
    const { app, status } = this.host;
    const view = status();
    app.set_scroll_y(Math.max(0, row - 3) * view.rowHeight);
    const es = app.row_start(row) ?? 0;
    this.panTo(es, width * 0.15);
    yield;
    const px = app.x_of(es) + 1; // a bar is at least 2 px wide
    const py = view.axisHeight + row * view.rowHeight - status().scrollY + view.rowHeight / 2;
    return { px, py };
  }

  /** A file's drag target, chosen by trial as the edit phase does: drag up to `DRAG_TRIALS` leaf tasks half a sweep, cancel each, keep the one that moves most rows. With `dragTarget` `critical` the one candidate is the first critical task, and it must move the project finish. */
  private *chooseDragRow(width: number, sweep: number): Generator<void, number> {
    const { input, status, app } = this.host;
    const leaves = dragCandidates(JSON.parse(app.table_rows(0, status().n)) as TableRow[], this.config!.dragTarget, (row) => app.row_start(row) ?? 0);
    const step = Math.max(1, Math.floor(leaves.length / DRAG_TRIALS));
    const before = this.starts(), finish = app.test_project_finish();
    let best = { row: -1, moved: 0 };
    for (const row of leaves.filter((_, i) => i % step === 0).slice(0, DRAG_TRIALS)) {
      const { px, py } = yield* this.aim(row, width);
      input.move(px, py);
      if (!status().hover) continue;
      input.down(px, py, false);
      this.stampedMove(px + 5 + sweep / 2, py);
      yield;
      const moved = this.config!.dragTarget === "critical" ? app.test_project_finish() - finish : this.movedFrom(before);
      input.cancel();
      yield;
      yield;
      if (moved > best.moved) best = { row, moved };
    }
    if (best.row < 0) throw new Error("no task on the file moves any row (with dragTarget=critical, the project finish) when dragged");
    return best.row;
  }

  private label(record: boolean): string {
    return record ? `repeat ${this.repeats.length + 1} of ${this.config!.repeats}` : "warm-up";
  }

  private *run(config: MeasureConfig): Generator<void, void> {
    const first = this.host.status();
    const tMid = (first.t[0] + first.t[1]) / 2;
    this.progress = "measuring the refresh period";
    if (config.baseline === 1) this.host.app.set_baseline();
    while (this.period === null) yield;
    for (let r = 0; r < config.warmup + config.repeats; r++) {
      const record = r >= config.warmup;
      const rec = yield* this.repeat(tMid, record);
      if (record) this.repeats.push(rec);
    }
    if (config.editRepeats > 0) {
      yield* this.editRun(config);
      yield* this.linkEditRun(config);
      yield* this.selectRun(config);
      yield* this.openRun(config);
    }
    this.progress = "writing the result";
  }

  private async finish(): Promise<MeasurementResult> { // oxlint-disable-line max-lines-per-function -- existing, over cap
    const config = this.config!;
    const period = this.period!;
    const pool = (pick: (r: RepeatRec) => PhaseRec): PhaseRec => poolRecs(this.repeats.map(pick));
    const zoom = pool((r) => r.zoom);
    const pan = pool((r) => r.pan);
    const drag = pool((r) => r.drag);
    const dragResponse = summarize(drag.responses);
    const verdicts = judge(period, zoom.result(period), pan.result(period), drag.result(period), dragResponse, this.drift.max);
    const environment = await this.environment(config, period);
    const warnings: string[] = [];
    if (environment.hardwareAcceleration === "off") warnings.push(`software rendering (${environment.hardwareAccelerationBasis}): frame times are not results`);
    if (environment.hardwareAcceleration === "unknown") warnings.push(`hardware acceleration unknown (${environment.hardwareAccelerationBasis}): frame times are not results until the renderer is known`);
    if (!environment.releaseBuild) warnings.push(`not a release build (wasm ${environment.build.wasm}, bundle ${environment.build.bundle})`);
    for (const [name, p] of [["zoom", zoom], ["pan", pan], ["drag", drag]] as const) {
      if (p.intervals.length === 0) warnings.push(`no ${name} frames were recorded`);
    }
    if (drag.recompute.length === 0) warnings.push("no drag recompute was recorded");
    for (const [name, w] of [["edit", this.editResult(period)?.afterEdit], ["link edit", this.linkEditResult(period)?.afterEdit], ["select", this.selectResult(period)], ["Fit", this.fitResult(period)]] as const) if (config.editRepeats > 0 && (w?.count ?? 0) === 0) warnings.push(`no ${name} frames were recorded`);
    const hz = 1000 / period;
    if (this.periodSource !== "refreshHz option" && !STANDARD_REFRESH_HZ.some((s) => Math.abs(hz - s) / s <= REFRESH_TOLERANCE)) {
      warnings.push(`the estimated refresh rate ${hz.toFixed(1)} Hz is not near a standard display rate (${STANDARD_REFRESH_HZ.join(", ")}): the thresholds use it as P; rerun with --param refreshHz=<display Hz>`);
    }
    const smooth = [...zoom.encode, ...pan.encode];
    const heap = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize;
    const result: MeasurementResult = {
      schema: 1,
      finishedAt: new Date().toISOString(),
      config: {
        warmup: config.warmup,
        repeats: config.repeats,
        dragMs: config.dragMs,
        panMs: config.panMs,
        dragDays: config.dragDays,
        dragTarget: config.dragTarget,
        editRepeats: config.editRepeats,
        baseline: config.baseline,
      },
      environment,
      zoom: zoom.result(period),
      pan: pan.result(period),
      drag: drag.result(period),
      edit: this.editResult(period),
      linkEdit: this.linkEditResult(period),
      select: this.selectResult(period), fit: this.fitResult(period), drawIn: this.openResult(period),
      dragResponseMs: dragResponse,
      recomputeMs: summarize(drag.recompute),
      encodeMs: summarize(smooth),
      wasm: { ...(await this.wasmSize()), memoryBytes: this.host.wasm.memory.buffer.byteLength },
      jsHeapBytes: heap ?? "unavailable",
      zoomAnchor: { steps: this.drift.steps, maxDriftPx: this.drift.max },
      repeats: this.repeats.map((r) => ({
        zoom: r.zoom.result(period),
        pan: r.pan.result(period),
        drag: r.drag.result(period),
        dragResponseMs: summarize(r.drag.responses),
        recomputeMs: summarize(r.drag.recompute),
      })),
      verdicts,
      verdict: verdicts.every((v) => v.verdict === "PASS") ? "PASS" : "MISS",
      warnings,
    };
    this.progress = `done: ${result.verdict}`;
    return result;
  }

  private async wasmSize(): Promise<{ rawBytes: number | "unavailable"; gzipBytes: number | "unavailable" }> {
    const entry = performance.getEntriesByType("resource").find((e) => /\.wasm(\?|$)/.test(e.name));
    if (!entry) return { rawBytes: "unavailable", gzipBytes: "unavailable" };
    try {
      const raw = await (await fetch(entry.name)).arrayBuffer();
      const zipped = new Response(new Blob([raw]).stream().pipeThrough(new CompressionStream("gzip")));
      return { rawBytes: raw.byteLength, gzipBytes: (await zipped.arrayBuffer()).byteLength };
    } catch {
      return { rawBytes: "unavailable", gzipBytes: "unavailable" };
    }
  }

  private async environment(config: MeasureConfig, period: number): Promise<Environment> {
    const { app, canvas } = this.host;
    const webgl = webglRenderer();
    const adapter = await webgpuAdapter();
    const backend = app.backend();
    const software = /swiftshader|llvmpipe|softpipe|software|basic render/i;
    const render = backend === "WebGPU" ? adapter : { text: webgl.text, fallback: false, known: webgl.known };
    // Software is certain only from a name or a fallback flag; "on" needs a named renderer.
    const hardwareAcceleration: Environment["hardwareAcceleration"] =
      render.fallback || software.test(render.text) ? "off" : render.known ? "on" : "unknown";
    const bundle = import.meta.env.PROD ? "production" : "development";
    const wasm = app.test_debug_build() ? "debug" : "release";
    const box = canvas.getBoundingClientRect();
    return {
      machine: config.machine,
      gpu: { webglRenderer: webgl.text, webgpuAdapter: adapter.text },
      browser: await browserVersion(),
      userAgent: navigator.userAgent,
      backend,
      webgpuFallback: this.host.fallback,
      hardwareAcceleration,
      hardwareAccelerationBasis: `${backend} renderer "${render.text}"${render.fallback ? " (a fallback adapter)" : ""}`,
      viewport: {
        canvasCssWidth: box.width,
        canvasCssHeight: box.height,
        windowInnerWidth: innerWidth,
        windowInnerHeight: innerHeight,
      },
      devicePixelRatio: devicePixelRatio,
      refreshRateHz: 1000 / period,
      refreshPeriodMs: period,
      refreshSource: this.periodSource,
      generatorSeed: app.test_seed(),
      fixture: config.scheduleSha256 === undefined ? "stress" : "file",
      scheduleSha256: config.scheduleSha256,
      tasks: app.task_count(),
      releaseBuild: wasm === "release" && bundle === "production",
      build: { wasm, bundle },
    };
  }
}
