// What the developer page (`main.ts`) hands the chart besides the schedule: its URL knobs. This module is not an
// export of the entry (`index.ts`), so a host page never sees these types, and the `entry` architecture check
// fails if the entry's exported types name anything from here or from `measure.ts`. The page sets the knobs
// before it calls `mount`; `mount` takes them once, as its first act, so a later `mount` gets none.
import type { Status } from "./api.js";
import type { MeasureConfig, MeasurementError, MeasurementResult } from "./measure.js";
import type { OpenTimings } from "./open-schedule.js";

/** What the developer page's status line reads about the run beside each frame's status: facts the public API does not give. */
export interface RunFacts {
  /** Milliseconds the open took: the first load, then each file opened in place. */
  loadMs: number;
  /** The importer's unsupported-feature lines; undefined for a generated schedule. */
  unsupported: string[] | undefined;
  /** The last file opened in place and the split of its open; null until one has been opened and has had its first frame. */
  open: { name: string; timings: OpenTimings } | null;
  /** Why WebGPU was abandoned for WebGL2, or null if it was not. */
  fallback: string | null;
  /** The table's rows: the line says how many are in the DOM. */
  tableRows: HTMLElement;
}

/** The developer page's status line (`status-line.ts`): the chart draws its text each frame while it is shown. */
export interface StatusLine {
  /** Put the line's element in the chart's root, after the canvas, hidden; a start (and a switch to WebGL2) calls it once. */
  place: (canvas: HTMLCanvasElement) => HTMLDivElement;
  /** The frame's line: the live numbers, from the status, the import notes' lines and the run's facts. */
  text: (s: Status, lines: string[], facts: RunFacts) => string;
}

/** The developer page's URL knobs. */
export interface PageKnobs {
  /** The page's query: read by `?panic=init` and echoed in a measured run's report. */
  params: URLSearchParams;
  /** Why WebGPU was abandoned for WebGL2 (`?fallback=`), or null if it was not. */
  fallback: string | null;
  /** The stress fixture's group count (`?groups=<k>`): a positive count builds it under a root summary with a `Phase` code of `k` values; unset is the plain fixture. */
  groups?: number;
  /** The scripted run to make (`?measure`), or null. */
  measure: MeasureConfig | null;
  /** The scripted run finished: its result, or why it could not run. */
  measured: (outcome: MeasurementResult | MeasurementError) => void;
  /**
   * Start the page again: the frame banner's Reload button, and a WebGPU frame that panicked (`why` is then the reason), which leaves the
   * module unusable, so the chart cannot switch to WebGL2 in place. A host page has no such knob, and decides for itself.
   */
  restart: (why: string | null) => void;
  /** The status line the `s` key shows, which only this page has; a host page hears `on("toggle-status")` for its own. */
  statusLine?: StatusLine;
}

let pending: PageKnobs | undefined;

/** Set the knobs the next `mount` takes. Only the developer page calls it. */
export function setPageKnobs(knobs: PageKnobs): void {
  pending = knobs;
}

/** The knobs set since the last take, or undefined (a host page sets none); the next take gets none. */
export function takePageKnobs(): PageKnobs | undefined {
  const knobs = pending;
  pending = undefined;
  return knobs;
}
