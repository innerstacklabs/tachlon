// The six read calls over a file, plain tasks or a document, in Node, with no page (gantt#1250): `openHeadless` reads an XER or MS Project file, plain `{ tasks, links }` or a `{ document }` with the engine and answers `whatIf`, `whatIfLonger`,
// `explain`, `health` and `criticalPath` as the chart's commands do, with the same result types, and `results`, every task's dates and float. No canvas, window or document is made or read.
import { readFileSync } from "node:fs";
import { Headless, initSync, LoadedSchedule } from "./wasm/app.js";
import { criticalPath } from "./critical-path.ts"; // `.ts`, so the Node tier can load this file; the types come from the `.js` names, which the declarations follow
import { explain } from "./explain.ts";
import { health } from "./health.ts";
import { whatIf, whatIfLonger } from "./what-if.ts";
import { fromTasks } from "./adapters/plain.ts";
import { callTool as run, toolDefinitions as definitions } from "./ai-tools.ts"; // the assistant's tools run over a `HeadlessSchedule` as over `chart.commands`; it has no `propose`, which answers `"unknownTool"`
export type { ToolDefinition, ToolResult } from "./ai-tools.js";
import type { ToolDefinition, ToolResult } from "./ai-tools.js";
import type { CriticalPathResult } from "./critical-path.js";
import type { ExplainResult } from "./explain.js";
import type { HealthResult } from "./health.js";
import type { WhatIfResult } from "./what-if.js";
import type { PlainCalendar, PlainLink, PlainTask } from "./adapters/plain.js";
import type { ScheduleOptions } from "./adapters/dhtmlx.js";
import type { HostCapabilities } from "./gantt-types.js";

/** What `openHeadless` reads: the bytes of a file, the plain `{ tasks, links }` the chart takes, or a schedule document (`toDocument`). */
export type HeadlessSource = Uint8Array | { tasks: readonly PlainTask[]; links?: readonly PlainLink[]; projectStart?: string; dataDate?: string; mustFinishBy?: string; options?: ScheduleOptions; calendars?: readonly PlainCalendar[]; durationUnit?: "days" | "hours" | "minutes"; lagCalendar?: "successor" | "predecessor" | "24h" | { calendar: string } } | { document: unknown };

/** Where the engine is: the file beside this script, which Node reads (the page fetches it instead). */
const ENGINE = new URL("./wasm/app_bg.wasm", import.meta.url);

let engineLoaded = false;
const freed = new WeakSet<HeadlessSchedule>();

// Declared here, not re-exported from `./ai-tools.ts`, so the emitted `headless.d.ts` names `.js` as the other declarations do.
export const toolDefinitions = (form: "mcp" | "anthropic" | "openai"): readonly ToolDefinition[] => definitions(form);
/** Run the assistant's tool `name` with `args` over `schedule`; once it is freed the answer is `{ ok: false, reason: "destroyed" }`, as for a destroyed chart, and no throw. */
export const callTool = (schedule: HeadlessSchedule, name: string, args: unknown): ToolResult =>
  freed.has(schedule) ? Object.freeze({ ok: false, reason: "destroyed", message: "The schedule has been freed." }) : run(schedule, name, args);
/**
 * One task's answer from `results()`, in decision 0017's names. `task` is the session slot, valid until the next open, so match an answer by `key` (the host's, or `null`); `code` is the Activity ID or `null`.
 * The dates are `YYYY-MM-DDTHH:mm`, the early `start` and `finish` and the late `lateStart` and `lateFinish`; `float` and `freeFloat` are working days of the task's calendar, `float` negative when a late limit cannot be met.
 */
export interface TaskResult {
  readonly task: number; readonly key: string | null; readonly code: string | null;
  readonly start: string; readonly finish: string; readonly lateStart: string; readonly lateFinish: string;
  readonly float: number; readonly freeFloat: number; readonly critical: boolean; readonly finished: boolean;
}

/** A schedule opened with no page. Each call reads the schedule and changes nothing, and answers as the chart's command of that name does (`results` has none). */
export interface HeadlessSchedule {
  /** The lines of the plain data that were not carried over, as `fromTasks` lists them, then one for each kind of change made to fit what the host stores (`capabilities`); empty for a file or a document opened with none. */
  readonly leftOut: readonly string[];
  /** What moves if each of `tasks` (up to 10) slips a week. */
  whatIf(tasks: readonly number[]): WhatIfResult;
  /** What moves if `task` takes `days` working days longer. */
  whatIfLonger(task: number, days: number): WhatIfResult;
  /** The tasks whose dependencies set `task`'s early start, nearest first. */
  explain(task: number): ExplainResult;
  /** The tasks behind each schedule health count. */
  health(): HealthResult;
  /** The critical tasks and the longest path's tasks. */
  criticalPath(): CriticalPathResult;
  /** Every task's early and late dates, total and free float, and critical flag, in one list that holds no summary and no deleted task. */
  results(): readonly TaskResult[];
  /** Release the engine's copy of the schedule, which JavaScript's collector does not; any call after `free()` throws. */
  free(): void;
}

/**
 * Open a schedule with no page from the bytes of an MS Project XML or Primavera P6 XER file, from plain `{ tasks, links }` as `fromTasks` reads it, or from `{ document }`.
 * `project` is `"all"` or a P6 `proj_id` for a file with several projects; with any other source it throws an `Error`.
 * Throws an `Error` with the reader's message when the file or document cannot be read, and the `PlainDataError` `fromTasks` throws for plain data it refuses (`name` is `"PlainDataError"`, `issues` lists the problems).
 */
export function openHeadless(source: HeadlessSource, project?: string, capabilities?: HostCapabilities): HeadlessSchedule {
  const file = source instanceof Uint8Array;
  if (project !== undefined && !file) throw new Error("project selects a project of a file; plain data and a document hold one schedule.");
  if (!engineLoaded) {
    initSync({ module: readFileSync(new URL(ENGINE)) });
    engineLoaded = true;
  }
  const plain = file || (typeof source === "object" && source !== null && !("tasks" in source) && "document" in source) ? null : fromTasks(source as Parameters<typeof fromTasks>[0]); // `tasks` before `document`, as `load` in `mount-load.ts` reads them; the data goes in uncopied, so `validate` sees what the caller passed
  const loaded = file ? LoadedSchedule.parse_bytes(source, project) : LoadedSchedule.parse_document(JSON.stringify(plain ? plain.document : (source as { document: unknown }).document));
  const read = (loaded.unsupported() as string[]).length;
  if (capabilities !== undefined) loaded.conform(JSON.stringify(capabilities));
  const fitted = (loaded.unsupported() as string[]).slice(read);
  const app = Headless.open(loaded);
  const schedule: HeadlessSchedule = Object.freeze({
    leftOut: Object.freeze([...(plain ? plain.leftOut : []), ...fitted]),
    whatIf: (tasks: readonly number[]) => whatIf(app, tasks),
    whatIfLonger: (task: number, days: number) => whatIfLonger(app, task, days),
    explain: (task: number) => explain(app, task),
    health: () => health(app),
    criticalPath: () => criticalPath(app),
    results: () => Object.freeze((JSON.parse(app.results()) as TaskResult[]).map((row) => Object.freeze(row))),
    free: () => (freed.add(schedule), app.free()),
  });
  return schedule;
}
