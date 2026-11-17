// The types that cross the WASM boundary, and the fence that keeps `test_*` exports out of the
// shell. Every file under packages/gantt/src except the Test-side files of docs/WASM_API.md types its `app`
// as `PublicApp`, so `app.test_x()` there fails `pnpm typecheck` with `Property 'test_x' does not
// exist on type 'PublicApp'`. This is the one non-test file that names the `test_` prefix, so it
// is on that list too (the list is the one `crates/app/tests/architecture/main.rs` reads).
import type { ZoomLevel } from "./view-state.js";
import type { App } from "./wasm/app.js";

/**
 * `App` without its `test_*` exports: what the shell, the toolbar and a host page may call.
 * @internal Left out of the package's typings (`tsconfig.lib.json`): it names the generated wasm types.
 */
export type PublicApp = Pick<App, Exclude<keyof App, `test_${string}`>>;

/** What `App.status()` returns. */
export interface Status {
  backend: string;
  /** Total task count, including tasks hidden by collapsed summaries. */
  n: number;
  /** Number of visible task rows. */
  rowCount: number;
  /** The leaf tasks the row filter and the lookahead window keep (collapse does not change it), or null while neither is on. */
  keptLeaves: number | null;
  /** While Longest path is on, what the line at the chart's bottom left names: the path's first and last activity, its activities and the calendar days from the first start to the project finish; else null. */
  path: { first: string; last: string; count: number; days: number } | null;
  /** Whether the critical-only filter is on. */
  criticalOnly: boolean;
  /** Whether the longest-path filter is on. */
  longestPath: boolean;
  filter: "off" | "critical" | "longest" | "slipped" | "joined" | "left" | "code" | "moved" | "near" | "status" | "check" | "float" | "constraint" | "resource";
  /** The lookahead window: `start` as `YYYY-MM-DD`. When `on` is false these are the defaults a reader gets (the data date's day, 3 weeks). */
  lookahead: { on: boolean; start: string; weeks: number };
  rows: [number, number];
  t: [number, number];
  from: string;
  to: string;
  /** The time axis's unit at the current zoom (`timeline::Level::name`). */
  level: ZoomLevel;
  pxPerMinute: number;
  scrollY: number;
  rowHeight: number;
  axisHeight: number;
  bars: number;
  criticalBars: number;
  /** Dependency arrows drawn this frame. */
  arrows: number;
  /** Non-working-day columns drawn this frame. */
  shaded: number;
  /** The file's data date in minutes from the origin Monday, or null if it has none. */
  dataDate: number | null;
  /** x of the data-date line on the canvas in CSS pixels, or null when it is not drawn. */
  dataDateX: number | null;
  /** Counts the changes the schedule has had; a drag that ends where it began leaves it as it was. */
  revision: number;
  /** Counts the solutions shown, restores included; the table redraws its rows when it moves. */
  stamp: number;
  /** Counts the files opened in place; the shell resets what it keeps for the old schedule when it moves. */
  epoch: number;
  frames: number;
  surface: [number, number];
  hover: Hover | null;
  /** The row the canvas draws as hovered: under its own pointer, else the one `set_hover_row` named; null if none. */
  hoverRow: number | null;
  drag: Drag | null;
  selected: Selected | null;
  /** The range selection as table rows `[first, last, extent]`: `first` to `last` inclusive, `extent` the end that moves; null with no range. */
  range: [number, number, number] | null;
  link: Link;
  /** What the pointer is doing; anything but `idle` keeps the chart's keys, editors and a second press out until the press ends. */
  gesture: Gesture;
  /** How the schedule differs from the baseline, or null with no baseline set. */
  baseline: BaselineSummary | null;
  /** The report of the last edit or drag release that moved other tasks, or null. */
  moved: Moved | null;
  /** The slide of the bars after an edit: whether one runs, and how long its copy took. */
  slide: SlideStatus;
  /** The nearest work before and after the window, as short dates, when no visible row has work in it; else null. */
  offscreen: { b: string | null; a: string | null } | null;
}

/** The slide of the bars to their new places after an edit. */
export interface SlideStatus {
  /** True while a slide runs. Never true while the page asked for reduced motion. */
  active: boolean;
  /** How long the copy of the last slide took, in milliseconds; null before the first slide. It stays after the slide ends. */
  copyMs: number | null;
}

/** How the schedule differs from the baseline, as counts. A task counts only if it is live now and was live in the baseline, except `added` and `removed`. */
export interface BaselineSummary {
  /** Tasks whose early finish is later than the baseline's. */
  slipped: number;
  /** Calendar days the project finish moved; negative when earlier. */
  finish_days: number;
  /** Tasks that have zero float now and had some in the baseline. */
  joined: number;
  /** Tasks that had zero float in the baseline and have some now. */
  left: number;
  /** Live tasks the baseline has no entry for (inserted since). */
  added: number;
  /** Tasks the baseline holds that are deleted now. */
  removed: number;
}

/** What an edit or a drag moved besides the edited task. */
export interface Moved {
  /** Counts reports, so a new one differs from the one already shown. */
  seq: number;
  /** For example `8 tasks moved · finish +7 days`. */
  text: string;
  /** Tasks other than the edited one whose early start or finish changed. */
  others: number;
  /** Calendar days the project finish moved; negative when earlier. */
  finish_days: number;
  /** 1 while the report is fully shown, then falls to 0 as it fades. */
  alpha: number;
  /** True for an edit, false for a drag release whose bars are already where the pointer left them. */
  slide: boolean;
}

/** What the pointer is doing: a press on a bar, a bar's right edge or a link terminal, the drag it became, or a press on a finished task's bar. */
export type Gesture = "idle" | "bar-pressed" | "bar-dragging" | "link-pressed" | "link-dragging" | "finished-pressed" | "resize-pressed" | "resize-dragging";

/** The selected link, the link drag in progress, and why the last link drag added nothing. */
export interface Link {
  /** The selected link's id (a dependency slot), or null. */
  selected: number | null;
  /** A link drag that has moved past the threshold, or null. */
  drag: LinkDrag | null;
  /** The message the last link drag was refused with, until the next press; null if it was not refused. */
  refusal: string | null;
  /** A calm note about a bar drag that moved nothing (a finished task), until the next press; null if none. */
  note: string | null;
}

/** A link drag in progress, from the bar of task `pred`. */
export interface LinkDrag {
  pred: number;
  /** The task under the pointer, or null. */
  target: number | null;
  /** The message a release on `target` would be refused with (a summary, a repeat or a loop), or null. */
  refused: string | null;
}

/** A selected task and counts for its driving path. */
export interface Selected {
  /** The selected task. */
  task: number;
  /** The selected task's timeline row. */
  row: number;
  /** Number of driving predecessors retained. */
  backCount: number;
  /** Number of driven successors retained. */
  forwardCount: number;
  /** More driving predecessors exist than were retained. */
  backCapped: boolean;
  /** More driven successors exist than were retained. */
  forwardCapped: boolean;
}

/** The last bar drag's result. */
export interface Drag {
  task: number;
  /**
   * First admissible start at or after where the pointer asked, minutes. A
   * task starts on a working instant; a milestone may also start at the end
   * of a working period.
   */
  wanted: number;
  /** Start the schedule gave, minutes. */
  es: number;
  /** Predecessors hold the task later than the pointer asks. */
  held: boolean;
  recomputeMs: number;
  /** The drag is still in progress. */
  active: boolean;
}

/** The task under the pointer. */
export interface Hover {
  task: number;
  es: number;
  ef: number;
  float?: number;
  critical: boolean;
  summary?: true;
  /** The pointer is on a leaf bar's finish edge, where a drag sets the duration. */
  edge?: true;
}

/** What `App.task_details()` returns for a scheduled task. */
export interface TaskDetails {
  task: number;
  name: string;
  outlinePath: string;
  /** The activity ID; empty when the task has none. */
  activityId: string;
  /** `Not started`, `In progress` or `Complete`. */
  status: string;
  /** Empty until set. */
  actualStart: string;
  /** Empty until set. */
  actualFinish: string;
  /** Empty for work not started. */
  percentComplete: string;
  start: string;
  finish: string;
  lateStart: string;
  lateFinish: string;
  floatMinutes: number;
  floatDays: number;
  floatHours: number;
  floatRemMinutes: number;
  floatText: string;
  critical: boolean;
  summary?: boolean;
  predecessorCount: number;
  predecessors: string[];
  successorCount: number;
  successors: string[];
  /** The names of the task's resources, in the file's order; empty for a task with none. */
  resources: string[];
  /** The baseline's start date. Written for a leaf with a baseline entry, unless the baseline's date is before the origin. */
  baselineStart?: string;
  /** The baseline's finish date; written as `baselineStart` is. */
  baselineFinish?: string;
  /** The start and finish variance in working days (`+5d`, `-2d`, `0`), each null when the baseline's date is before the origin. Written for a leaf with a baseline entry. */
  baselineVariance?: [string | null, string | null];
}

/** One table row from `App.table_rows`: task, start, finish, float, critical, and kind (leaf 0, expanded summary 1, collapsed summary 2), then the Start, Finish and Float variance against the baseline as signed working-day text (`+5d`, `-2d`, `0`), or null for a summary, a task the baseline lacks, a session with no baseline, a baseline from a file with no float (the Float cell) or with a date before the origin (that Start or Finish cell) and while a bar is dragged. Duration is read separately. */
export type TableRow = [number, string, string, number | null, boolean, 0 | 1 | 2, string | null, string | null, string | null];

/** A group header from `App.table_rows` while the rows are grouped: no task, the code value (null for the leaves with no value), the leaf count, and kind 3 (open) or 4 (closed). */
export type HeaderRow = [null, string | null, number, null, false, 3 | 4, null, null, null];

/** What `App.print_pages(title, printed_on, page_size?, measure?, columns?, from?, to?, layout?)` returns, where `layout` is the name of the print layout in use, headed `Layout: name`, and the optional `page_size` is `"letter"` (the default) or `"a4"`, `columns` names the list columns to print, in order (`print-sheet.ts` passes the screen's), and `from` and `to` (days, `YYYY-MM-DD`, given together) print only the rows active between them, the pages scaled to that window and headed `Dates: from to to`: one SVG document per printed page, in page order, at least one. */
export type PrintPages = string[];

/** The paper sizes `App.print_pages` lays out and `Gantt.printOn` prints on. */
export type PageSize = "letter" | "a4";
