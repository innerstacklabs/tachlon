// What a side of the benchmark supplies to the driver: four hooks, and seven optional ones.
//
// A side is a module whose default export is a `Side`. Each hook runs in the page (Playwright
// sends its source text), so it is an arrow function that closes over nothing and reads only what
// the page shows. The driver never reaches into a side's code beyond these. All positions are CSS
// pixels of the page, as Playwright's mouse takes them.
//
// Load is also split by two performance marks the page itself sets, the same names on every side:
// `library-ready` once the component's code is loaded and can run (ours: the wasm instantiated; a
// script library: its module evaluated), and `data-start` when the page starts fetching the schedule.

/** A box on the page. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Where the wheel goes, and how far the view can scroll from where it is. */
export interface ScrollTarget extends Box {
  /** Rows scrolled from the top, px. */
  top: number;
  /** Time scrolled from the side's own origin, px, growing toward later dates. Only its change means anything. */
  left: number;
  /** The furthest `top` can go, px. */
  maxTop: number;
  /** The furthest `left` can go, px, on the same scale as `left`; Infinity for a time axis the wheel never stops at. */
  maxLeft: number;
}

/** A task bar the side draws (not a summary's): its box on the page, and the task it is, when the side names it. */
export interface LeafBar extends Box {
  /** The side's own name for the task, which `dragState` reads. */
  id?: string;
  /** The task's id in the benchmark dataset: the driver records it, and the table refuses a pair whose sides dragged different tasks. */
  datasetId?: string;
}

/** The task bars the page draws now, and the width of one day on its time axis, px. */
export interface LeafBars {
  bars: LeafBar[];
  pxPerDay: number;
}

/** The bar the driver drags (`grab.ts` picks it from `leafBars`): its centre, the width of one day, and the task it is, when the side names it. */
export interface DragTarget {
  x: number;
  y: number;
  pxPerDay: number;
  id?: string;
  datasetId?: string;
}

export interface Side {
  /** A neutral name for the result file. */
  name: string;
  /** True once a frame shows the first row and its bar, with exactly `rows` tasks loaded. */
  ready: (rows: number) => boolean;
  scrollTarget: () => ScrollTarget;
  /**
   * Every task bar the page draws now, summaries left out, wherever it lies: the driver, not the
   * side, picks the one to drag, by the same rule on every side (`grab.ts`).
   */
  leafBars: () => LeafBars | Promise<LeafBars>;
  /**
   * Text that changes once a drag of `target` has been committed (ours: the schedule's revision; a side
   * that names the task: its start date). Read before the press, and after the release every frame
   * until it changes, so a side that commits after the release is timed until it has.
   */
  dragState: (target: DragTarget) => string;
  /** The renderer the side drew with (ours: WebGPU or WebGL2), when it says. */
  backend?: () => string | null;
  /** True when the page is not the side's production build (a development, debug or unminified trial build). */
  debugBuild?: () => boolean;
  /**
   * The SHA-256 (hex) of the benchmark `dataset.json` the schedule on the page was made from, as
   * the side's own adapter recorded it beside the file it loads. The table refuses a run that
   * does not report it, or reports another cell's: the row count alone does not prove the schedule.
   */
  datasetSha256?: () => string | null | Promise<string | null>;
  /** The bytes of the file the page loaded (each side loads its own file of the one dataset); the table's note gives them. */
  inputBytes?: () => number | null | Promise<number | null>;
  /** What build the page runs, as a table labels it (for example a vendor's version and "trial build"). */
  build?: () => string | null;
  /** How many whole rows the view shows at once. */
  rowsOnScreen?: () => number | null;
  /** Null when a sample of the dates on the page is the schedule's; else why not. A failed check ends the run at load. */
  spotCheck?: () => string | null | Promise<string | null>;
  /**
   * When the work the page's load started is done, as `performance.now()` ms on the page; null until
   * then. For a side whose ready frame can come first (a schedule drawn from saved dates while its
   * engine still computes); the driver waits for it before the input phases and records it.
   */
  settled?: () => number | null;
}
