// The host commands for find, the outline levels, the rows, a link, the data date and the baseline (gs#1195): what the toolbar, the
// find box (apps/chrome) and the baseline buttons call, as `Gantt.commands` offers it. Each is the same app call the chart's own control
// makes. A bad input or a refusal is a result (`false`, or an `EditResult` with its reason and message), never a throw.
import type { PublicApp } from "./api.js";
import { setBaselineFromFile } from "./baseline-from-file.js";
import { type AcceptResult, type CompareResult, type EditResult, type LinkResult, readAnswer, refused, type RowResult } from "./edit-result.js";
import { whatIf, whatIfLonger, type WhatIfResult } from "./what-if.js";
import { explain, type ExplainResult } from "./explain.js";
import { health, type HealthResult } from "./health.js";
import { criticalPath, type CriticalPathResult } from "./critical-path.js";
import { acceptKey, propose, type ProposedChange, type ProposedKey, rejectKey, type ProposeResult } from "./propose.js";

/** The deepest outline level `collapseToLevel` takes: the largest u32. */
const MAX_LEVEL = 0xffff_ffff;

/** The commands this file adds to `Commands`; `commands.ts` documents each. */
export interface HostCommands {
  find(query: string): number | false | undefined;
  findNext(): number | false | undefined;
  findGo(index: number): boolean | undefined;
  collapseToLevel(level: number): boolean | undefined;
  expandAll(): boolean | undefined;
  insertRow(): RowResult | undefined;
  assignKey(record: { task: number } | { link: number }, key: string): EditResult | undefined;
  deleteRow(): RowResult | undefined;
  deleteLink(): LinkResult | undefined;
  setDataDate(day: string | null): EditResult | undefined;
  setBaseline(records?: readonly { key: string; start: string; finish: string }[]): EditResult | undefined;
  setOverlay(records: readonly { key: string; start: string; finish: string }[] | null): EditResult | undefined;
  clearBaseline(): EditResult | undefined;
  addNewToBaseline(): EditResult | undefined;
  setBaselineFromFile(file: File): Promise<CompareResult>;
  whatIf(tasks: readonly number[]): WhatIfResult | undefined;
  whatIfLonger(task: number, days: number): WhatIfResult | undefined;
  explain(task: number): ExplainResult | undefined;
  health(): HealthResult | undefined;
  criticalPath(): CriticalPathResult | undefined;
  propose(change: ProposedChange): ProposeResult | undefined;
  acceptProposal(): AcceptResult | undefined;
  rejectProposal(): boolean | undefined;
  acceptProposed(key: ProposedKey): AcceptResult | undefined;
  rejectProposed(key: ProposedKey): boolean | undefined;
}

/** The records as the JSON text the app reads, or undefined for anything but an array JSON can hold (a BigInt or a cycle). */
const recordsJson = (records: unknown): string | undefined => {
  try { return Array.isArray(records) ? JSON.stringify(records) : undefined; } catch { return undefined; }
};

/** What `accept` (a call of an `accept_*` export) answers as an `AcceptResult`, or `"busy"` without calling it. */
const accepted = (busy: () => boolean, accept: () => string): AcceptResult => {
  if (busy()) return refused("busy");
  const answer = readAnswer(accept());
  return answer.ok ? Object.freeze({ ok: true, changes: answer.changes ?? 0 }) : answer;
};

/** `busy` is `Gantt.busy()`: a cell edit, or a press or drag the app reports; `reason` turns a thrown value into one line. */
export function createHostCommands(app: PublicApp, busy: () => boolean, reason: (err: unknown) => string): HostCommands { // oxlint-disable-line max-lines-per-function -- existing, over cap
  // The host's own search: its query (null before the first `find`), its matches, the stamp they were read at, and the match it went
  // to (-1 before the first) with its task. Every caller of `find` comes through these commands (the chrome's find box among them),
  // so the app's match list is the query's; an edit, an insert or a filter moves the stamp, and only then is the query run again.
  let query: string | null = null;
  let tasks: number[] = [];
  let stamp = -1;
  let at = -1;
  let task = -1;
  const read = (): { stamp: number; tasks: number[] } => JSON.parse(app.find_found()) as { stamp: number; tasks: number[] };
  const rerun = (): number => {
    if (query === null) return 0;
    let found = read();
    if (found.stamp !== stamp) {
      app.find(query);
      found = read();
    }
    ({ stamp, tasks } = found);
    // Continue from the task gone to, wherever an insert moved it; with it gone, from the match before its place.
    const moved = tasks.indexOf(task);
    at = moved >= 0 ? moved : task < 0 ? -1 : Math.min(at, tasks.length) - 1;
    task = tasks[at] ?? -1;
    return tasks.length;
  };
  const goTo = (index: number): boolean => {
    if (index >= tasks.length || app.find_go(index) === undefined) return false;
    at = index;
    task = tasks[index] ?? -1;
    return true;
  };
  const go = (index: number): boolean => Number.isInteger(index) && index >= 0 && !busy() && rerun() > 0 && goTo(index);
  const edit = (run: () => string, guarded = true): EditResult => {
    if (guarded && busy()) return refused("busy");
    const answer = readAnswer(run());
    return answer.ok ? Object.freeze({ ok: true }) : answer;
  };
  const row = (run: () => string): RowResult => {
    if (busy()) return refused("busy");
    const answer = readAnswer(run());
    return answer.ok ? Object.freeze({ ok: true, task: answer.task ?? -1 }) : answer;
  };
  const level = (run: () => boolean): boolean => !busy() && run();
  return {
    find(text) {
      if (typeof text !== "string") return false;
      query = text;
      stamp = -1;
      at = task = -1;
      return rerun();
    },
    findNext() {
      if (busy() || rerun() === 0) return false;
      const next = (at + 1) % tasks.length;
      return goTo(next) ? next : false;
    },
    findGo: go,
    // Checked before the number reaches the module's u32, which would wrap a larger one.
    collapseToLevel: (n) => Number.isInteger(n) && n >= 1 && n <= MAX_LEVEL && level(() => app.collapse_to_level(n)),
    expandAll: () => level(() => app.expand_all()),
    insertRow: () => row(() => app.try_insert_row()),
    deleteRow: () => row(() => app.try_delete_row()),
    // Not an edit, so not refused while busy. A bad shape or key goes to the app as a slot or key it refuses by name.
    assignKey(record, key) {
      const link = typeof record === "object" && record !== null && "link" in record && !("task" in record);
      const slot = link ? (record as { link: number }).link : (record as { task?: number } | null)?.task;
      const whole = typeof slot === "number" && Number.isInteger(slot) && slot >= 0 && slot <= MAX_LEVEL;
      const text = typeof key === "string" ? key : "";
      return edit(() => (link ? app.assign_link_key(whole ? slot : MAX_LEVEL, text) : app.assign_task_key(whole ? slot : MAX_LEVEL, text)), false);
    },
    deleteLink() {
      if (busy()) return refused("busy");
      const answer = readAnswer(app.try_delete_link());
      return answer.ok ? Object.freeze({ ok: true, link: answer.link ?? -1 }) : answer;
    },
    setDataDate(day) {
      // Only null clears: an empty or blank text is not a day, though the app's own box reads it as clear.
      if (day !== null && (typeof day !== "string" || day.trim() === "")) return refused("badDate");
      if (busy()) return refused("busy");
      // The day already shown changes nothing: `state().dataDate` drops a file's time of day, so setting it back must not
      // move an 08:00 data date to 00:00, recompute, add an undo step and mark the schedule unsaved.
      if (day !== null && day.trim() === app.data_date_text().slice(0, 10)) return Object.freeze({ ok: true });
      return edit(() => app.try_set_data_date(day ?? ""));
    },
    setBaseline(records) {
      const json = recordsJson(records);
      if (records === undefined) return edit(() => app.try_set_baseline());
      return json === undefined ? Object.freeze({ ok: false, reason: "badRecords", message: "setBaseline takes an array of records." }) : edit(() => app.try_set_baseline_records(json));
    },
    // Not an edit, so not refused while busy: the overlay does not move while a bar is dragged.
    setOverlay(records) {
      const json = recordsJson(records);
      if (records === null) return edit(() => app.try_set_overlay_records(undefined), false);
      return json === undefined ? Object.freeze({ ok: false, reason: "badRecords", message: "setOverlay takes an array of records, or null." }) : edit(() => app.try_set_overlay_records(json), false);
    },
    clearBaseline: () => edit(() => app.try_clear_baseline()),
    addNewToBaseline: () => edit(() => app.try_add_new_to_baseline()),
    setBaselineFromFile: async (file) => (busy() ? refused("busy") : setBaselineFromFile(app, file, reason)),
    whatIf: (tasks) => whatIf(app, tasks),
    whatIfLonger: (task, days) => whatIfLonger(app, task, days),
    explain: (task) => explain(app, task),
    health: () => health(app),
    criticalPath: () => criticalPath(app),
    propose: (change) => propose(app, change),
    acceptProposal: () => accepted(busy, () => app.accept_proposal()),
    rejectProposal: () => app.reject_proposal(),
    acceptProposed: (key) => accepted(busy, () => acceptKey(app, key)),
    rejectProposed: (key) => rejectKey(app, key),
  };
}
