// The measured run's configuration and the host the script drives (plan gs#1,
// "Demo and measurements (M3)"). Part of the harness in `../measure.ts`.
import type { InitOutput } from "../wasm/app.js";
import type { App } from "../wasm/app.js";
import type { PublicApp, Status } from "../api.js";
import type { Environment as StressEnvironment, MeasurementResult as StressResult } from "./result.js";

/** The run's environment: `fixture` is "file" for `?schedule`, and then `scheduleSha256` is the hash of the file. */
export type Environment = Omit<StressEnvironment, "fixture"> & { fixture: "stress" | "file"; scheduleSha256?: string };
export type MeasurementResult = Omit<StressResult, "environment"> & { environment: Environment };

/** The page's input functions: the DOM listeners and the script both call these. */
export interface Input {
  wheel(x: number, y: number, dx: number, dy: number, zoom: boolean): void;
  down(x: number, y: number, alt: boolean): void;
  /** `at` is the pointer event's `timeStamp` (`performance.now()` clock); a move without one is not a drag-response sample. */
  move(x: number, y: number, at?: number): void;
  up(): void;
  cancel(): void;
}

/** @internal */
export interface Host {
  /** The full type: `measure.ts` is Test-side and calls `test_*`. */
  app: App;
  canvas: HTMLCanvasElement;
  status: () => Status;
  input: Input;
  params: URLSearchParams;
  wasm: InitOutput;
  /** Why WebGPU was abandoned for WebGL2, or null if it was not. */
  fallback: string | null;
}

/**
 * What the shell hands over: the same `Host`, with `app` typed `PublicApp` so that `main.ts` never holds the full type. `Metrics` widens it.
 * @internal
 */
export type HostInit = Omit<Host, "app"> & { app: PublicApp };

/** What a measured run does; URL options, with the plan's defaults. */
export interface MeasureConfig {
  /** Full runs of the script, discarded. */
  warmup: number;
  /** Runs recorded. */
  repeats: number;
  /** Length of the continuous drag, ms. */
  dragMs: number;
  /** Length of the pan, ms. */
  panMs: number;
  /**
   * Days the drag sweeps the head task across (the engine's `stress_drag`
   * benches move it 0 to 29 days). The drag view is zoomed to fit the sweep.
   */
  dragDays: number;
  /** What the drag moves: `head` the stress fixture's head task (a file's trial choice), `critical` the first critical leaf task, which moves the project finish. */
  dragTarget: "head" | "critical";
  /** Recorded edits of the edit phase (0 skips the phase). */
  editRepeats: number;
  /** 1 sets a baseline before the warm-up (the variance and comparison-bar costs need one); 0 sets none. */
  baseline: 0 | 1;
  /** Refresh rate to use instead of the estimate. */
  refreshHz: number | null;
  machine: string;
  /** SHA-256 (hex) of the `?schedule` file the run is on; set by the page, absent on the stress fixture. */
  scheduleSha256?: string;
}

function count(params: URLSearchParams, name: string, fallback: number, min: number): number {
  const raw = params.get(name);
  const v = raw === null ? fallback : Number(raw);
  return Number.isFinite(v) && v >= min ? v : fallback;
}

/** The run's configuration, or null when the page was not asked to measure. */
export function measureConfig(params: URLSearchParams): MeasureConfig | null {
  if (!params.has("measure")) return null;
  const hz = Number(params.get("refreshHz"));
  return {
    warmup: count(params, "warmup", 1, 0),
    repeats: count(params, "repeats", 5, 1),
    dragMs: count(params, "dragMs", 10_000, 100),
    panMs: count(params, "panMs", 3_000, 100),
    dragDays: count(params, "dragDays", 30, 1),
    dragTarget: params.get("dragTarget") === "critical" ? "critical" : "head",
    editRepeats: count(params, "editRepeats", 5, 0),
    baseline: count(params, "baseline", 0, 0) === 1 ? 1 : 0,
    refreshHz: params.has("refreshHz") && Number.isFinite(hz) && hz > 0 ? hz : null,
    machine: params.get("machine") ?? `${navigator.platform}, ${navigator.hardwareConcurrency} logical cores`,
  };
}
