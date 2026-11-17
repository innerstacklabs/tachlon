// The shape of a measured run's result (plan gs#1, "Demo and measurements (M3)"):
// what the page publishes as one JSON document. Part of the harness in `../measure.ts`.
import type { Check, FrameStats, Summary } from "../metrics.js";
import type { MeasureConfig } from "./config.js";

export interface Environment {
  machine: string;
  gpu: { webglRenderer: string; webgpuAdapter: string };
  browser: string;
  userAgent: string;
  backend: string;
  /** Why WebGPU was abandoned for WebGL2, or null. */
  webgpuFallback: string | null;
  /** "unknown" when the rendering adapter or renderer cannot be identified; never "on" by default. */
  hardwareAcceleration: "on" | "off" | "unknown";
  hardwareAccelerationBasis: string;
  viewport: { canvasCssWidth: number; canvasCssHeight: number; windowInnerWidth: number; windowInnerHeight: number };
  devicePixelRatio: number;
  refreshRateHz: number;
  refreshPeriodMs: number;
  refreshSource: "10th percentile of 90 idle frames" | "refreshHz option";
  generatorSeed: number;
  fixture: "stress";
  tasks: number;
  /** True for a release wasm built into a production bundle. */
  releaseBuild: boolean;
  build: { wasm: "release" | "debug"; bundle: "production" | "development" };
}

export interface PhaseResult extends FrameStats {
  /** CPU time of `App.frame()` (encode and submit, plus the recompute in a drag), ms. */
  encodeMs: Summary;
  /** Sum of the recorded frame intervals, ms. */
  totalMs: number;
  /** From the phase's first frame to the frame after its last, ms; equals `totalMs` when no final interval is dropped. */
  spanMs: number;
}

/** The edit phase: a scripted duration edit on a task with a wide cone, then an undo (`measure/edit-phase.ts`). */
export interface EditResult {
  /** The task the script chose, and its table row. */
  task: number;
  row: number;
  /** The duration text the edit set. */
  text: string;
  /** Rows whose start moved after the edit, in the recorded repeat that moved fewest. A phase that made no edit reads 0. */
  moved: number;
  movedPerRepeat: number[];
  /** What the script tried before choosing: the same edit on each candidate, undone, and the rows it moved. */
  candidates: { task: number; row: number; moved: number }[];
  /** The duration of the `App.set_duration` call, which blocks the page, ms. */
  setMs: Summary;
  /** The page's copy of the old and new dates for an edit (`status.slide.copyMs`), over the edits; null before the slide exists. */
  copyMs: Summary | null;
  /** Frames in the first 400 ms after each edit (the slide, 360 ms, plus a 40 ms margin), pooled over the edits. */
  slide: PhaseResult;
  /** Frames from 400 to 600 ms after each edit, pooled the same way. */
  afterEdit: PhaseResult;
  /** Frames in 600 ms of no edit, pooled the same way. */
  noEdit: PhaseResult;
}

/** One step of the link-edit scenario: the frames 0 to 400 ms after it and 400 to 600 ms after it, pooled over the edits. */
export interface WindowPair {
  slide: PhaseResult;
  afterEdit: PhaseResult;
}

/**
 * The link-edit phase: a scripted link added by the pointer path (an Alt-drag from one leaf bar onto another), then removed
 * with `App.remove_selected_link`, then undone and redone (`measure/link-edit-phase.ts`). `slide` and `afterEdit` are the
 * windows after the add; the rows are in the shape of `EditResult`.
 */
export interface LinkEditResult {
  /** The task the link starts at, and the task it ends at (the successor whose cone moves), each with its table row. */
  pred: number;
  predRow: number;
  task: number;
  row: number;
  /** Rows whose start moved after the add, in the recorded repeat that moved fewest. */
  moved: number;
  movedPerRepeat: number[];
  /** What the script tried before choosing: the same add on each pair, undone, and the rows it moved. A pair the engine refused (a loop, a duplicate) reads 0. */
  candidates: { pred: number; task: number; predRow: number; row: number; moved: number }[];
  /** Pairs the engine refused, which the choice skipped. */
  refused: number;
  /** The duration of the drop (`App.pointer_up` of the link drag), which adds the link and blocks the page, ms. */
  setMs: Summary;
  /** The page's copy of the old and new dates for the add (`status.slide.copyMs`), over the edits; null before the slide exists. */
  copyMs: Summary | null;
  /** Frames in the first 400 ms after each add, and from 400 to 600 ms, pooled over the edits. */
  slide: PhaseResult;
  afterEdit: PhaseResult;
  /** The same two windows after `App.remove_selected_link`. */
  remove: WindowPair;
  /** After the undo of the removal and after the redo: `slide` pools the 400 ms after each, `afterEdit` is the 200 ms after the redo. */
  undoRedo: WindowPair;
  /** Frames in 600 ms of no edit, pooled the same way. */
  noEdit: PhaseResult;
}

/** The select phase: 400 ms of frames after selecting a task with a long driving chain (`measure/select-phase.ts`). */
export interface SelectResult extends PhaseResult {
  /** The task the script chose, its table row and the length of its driving chain behind it. */
  task: number;
  row: number;
  back: number;
  /** What the script tried before choosing: each candidate selected, cleared, and the chain it showed. */
  candidates: { task: number; row: number; back: number }[];
}

/** The Fit phase: 400 ms of frames after `App.fit_project` from a zoomed-in view, pooled over the repeats. */
export interface FitResult extends PhaseResult {
  /** The scale (px per minute) before and after Fit, one per recorded repeat. */
  pxPerMinuteBefore: number[];
  pxPerMinuteAfter: number[];
}

/** The draw-in phase: 400 ms of frames after `App.open` of the loaded schedule, pooled over the repeats (`measure/open-phase.ts`). */
export type OpenResult = PhaseResult;

export interface Repeat {
  zoom: PhaseResult;
  pan: PhaseResult;
  drag: PhaseResult;
  dragResponseMs: Summary;
  recomputeMs: Summary;
}

export interface MeasurementResult {
  schema: 1;
  finishedAt: string;
  config: Omit<MeasureConfig, "refreshHz" | "machine">;
  environment: Environment;
  /** Phases pooled over the recorded repeats; the verdicts judge these. */
  zoom: PhaseResult;
  pan: PhaseResult;
  drag: PhaseResult;
  /** The edit phase, or null when `editRepeats` is 0. */
  edit: EditResult | null;
  /** The link-edit phase, or null when `editRepeats` is 0. A result written before the phase existed has no such key. */
  linkEdit?: LinkEditResult | null;
  /** The select phase, or null when `editRepeats` is 0. A result written before it existed has no such key. */
  select?: SelectResult | null;
  /** The Fit phase, or null when `editRepeats` is 0. A result written before it existed has no such key. */
  fit?: FitResult | null;
  /** The open draw-in phase (not `open`, the open-time block), or null when `editRepeats` is 0. A result written before it existed has no such key. */
  drawIn?: OpenResult | null;
  /** Pointer event to the requestAnimationFrame after its recompute, table update and draw, ms. */
  dragResponseMs: Summary;
  /** The WASM recompute alone (50k tasks, one drag step), ms. */
  recomputeMs: Summary;
  /** CPU encode and submit per frame over zoom and pan frames, which recompute nothing, ms. */
  encodeMs: Summary;
  wasm: { rawBytes: number | "unavailable"; gzipBytes: number | "unavailable"; memoryBytes: number };
  /** `performance.memory.usedJSHeapSize` at the end, where the browser exposes it. */
  jsHeapBytes: number | "unavailable";
  zoomAnchor: { steps: number; maxDriftPx: number };
  repeats: Repeat[];
  verdicts: Check[];
  /** PASS when every verdict is PASS. A MISS is a result, not a failure. */
  verdict: "PASS" | "MISS";
  /** Reasons the numbers are not results (software rendering, a debug build). */
  warnings: string[];
}

/** The page reports this instead of a result when the script cannot run. */
export interface MeasurementError {
  error: string;
}
