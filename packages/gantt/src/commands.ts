// The host commands (gs#1085): what a page's own buttons call to drive the chart. Each maps to an export of the
// WASM module that the toolbar and the keys already use, so a host button and the chart's own control do the same thing.
import { type Shell } from "./shell.js";
import { type Filter, setFilter } from "./set-filter.js";
import { type AcceptResult, type CompareResult, type EditResult, type LinkResult, type Refusal, refused, type RowResult } from "./edit-result.js";
import { createHostCommands } from "./host-commands.js";
import { type WhatIfResult } from "./what-if.js";
import { type ExplainResult } from "./explain.js";
import { type HealthResult } from "./health.js";
import { type CriticalPathResult } from "./critical-path.js";
import { type ProposedChange, type ProposedKey, type ProposeResult } from "./propose.js";
import { applyColumns, printColumns } from "./print-sheet.js";
import type { PageSize } from "./api.js";

/** One zoom step, about 2x: the same wheel distance the toolbar's buttons and the `+` and `-` keys send. */
const ZOOM_STEP_DY = 350;

/** Where `scrollTo` goes: a task (its slot number, or its key as text), or a day (`YYYY-MM-DD`). */
export type ScrollTarget = { task: number | string } | { date: string };

/** What a host page may call to drive the chart. After `Gantt.destroy()` every method does nothing and returns undefined; none throws. */
export interface Commands {
  /** Zoom in one step about the middle of the chart. */
  zoomIn(): void;
  /** Zoom out one step about the middle of the chart. */
  zoomOut(): void;
  /** Show the whole project, as the Fit button does. False, changing nothing, with no task. */
  fit(): boolean | undefined;
  /**
   * Bring a task's row and bar into view (it selects the task), or show a day. A text `task` is the host's key, a number the chart's own task number. False, changing nothing, for a task
   * that is deleted, unknown (a key no live task holds) or has no row (hidden by a collapsed summary or the row filter), for a day that is not a date, and while a bar is pressed or dragged.
   */
  scrollTo(target: ScrollTarget): boolean | undefined;
  /** The same call as `scrollTo({ task })`: select `task` (a text is the host's key, a number the chart's own task number) and bring its row and bar into view, as a click on its row does. False, changing nothing, in the cases `scrollTo({ task })` refuses. */
  select(task: number | string): boolean | undefined;
  /**
   * Turn a filter on, as the toolbar, the Filters menu and the keys do: a row filter (`"critical"`, `"longestPath"`,
   * `"nearCritical"`, `"slipped"`, `"floatLost"`, `"joinedCritical"`, `"leftCritical"`, `"moved"`, `"startConstraint"`, `{ status }`, `{ code }`, `{ resource }`) replaces the one that is on, `"off"` turns
   * the row filter off, and `{ lookahead }` sets or (with null) removes the window, which combines with the row filter. True when
   * the chart took it, also when it was already so; false, changing nothing, when it refused: no outline, a bar press, a value it
   * does not know, Slipped, Joined or Left with no baseline, Float lost with no baseline float, Show moved with no move report.
   */
  setFilter(filter: Filter): boolean | undefined;
  /** Turn the row filter and the lookahead window off together, as the Filters menu's Clear does. False when the filters cannot change now. */
  clearFilters(): boolean | undefined;
  /**
   * Group the rows under one header per value of an activity code type (`filterInputs().activityCodes` lists the types), as the Filters menu's Group choice does, or with null show the outline again.
   * The grouping is view state (`state().groupBy`): a saved document does not carry it and an undo does not undo it. False, changing nothing, for a type no task carries, with no outline, and while a bar is pressed or dragged.
   */
  setGroupBy(codeType: string | null): boolean | undefined;
  /** Undo the latest edit. True means an edit was undone; a failure is reported on the console and gives false. */
  undo(): boolean | undefined;
  /** Redo the latest undone edit. True means an edit was redone; a failure is reported on the console and gives false. */
  redo(): boolean | undefined;
  /**
   * Search the task names and outline paths for `query` (case-blind; with a row filter or the lookahead window on, only the
   * shown rows), as the find box does, and return the number of matches. It selects nothing: `findGo` and `findNext` do.
   * False for a query that is not a string. `findGo` and `findNext` run the last `find`'s query again when the schedule or the
   * filters changed since (`Status.stamp` moved), so they go only to its matches; the chrome's find box searches through this same call.
   */
  find(query: string): number | false | undefined;
  /** Go to the match after the task last gone to (the first after a `find`), wrapping, and return its index: an insert before that task skips nothing. False with no match, and while busy. */
  findNext(): number | false | undefined;
  /** Select match `index` (from 0) of the last `find` and bring its row into view. False for an index outside the matches or not a whole number, and while busy. */
  findGo(index: number): boolean | undefined;
  /** Collapse every summary at outline level `level` and deeper (1 is the top), as the toolbar's level buttons do. False, changing nothing, for a level that is not a whole number from 1, with no outline, and while busy. */
  collapseToLevel(level: number): boolean | undefined;
  /** Expand every summary. False with no outline, and while busy. */
  expandAll(): boolean | undefined;
  /** Insert a task named `New task` below the selected task's subtree (the last top-level row with nothing selected) and select it, as the Insert key does: `{ ok: true, task }`, or `{ ok: false, reason }` changing nothing. One undo step. */
  insertRow(): RowResult | undefined;
  /** Hand back the key the host gave a task or link the chart made, after it saved the row: `{ ok: true }`, or `{ ok: false, reason, message }` changing nothing (`"keyTaken"`, `"hasKey"`, `"emptyKey"`, `"unknownTask"`, `"unknownLink"`). Not an edit: no undo step, no revision, and the chart does not mark the schedule unsaved. `{ task }` and `{ link }` are the slots `insertRow` and a drawn link reported; a slot is valid only until the next load, open or remount. The same key again answers ok. */
  assignKey(record: { task: number } | { link: number }, key: string): EditResult | undefined;
  /** Delete the selected task (never a selected link), as the Delete key does, and select the next row: `{ ok: true, task }` with the deleted task, or `{ ok: false, reason }` changing nothing. One undo step. */
  deleteRow(): RowResult | undefined;
  /** Remove the selected link, as the toolbar's Delete does when a link is selected: `{ ok: true, link }` with the removed link, or `{ ok: false, reason, message }` changing nothing (`"noSelection"` with no link selected). One undo step. */
  deleteLink(): LinkResult | undefined;
  /** Move the data date to 00:00 on `day`, written `YYYY-MM-DD`, and recompute, or clear it with null. `{ ok: true }`, or `{ ok: false, reason }` changing nothing (`"badDate"` for any other value). One undo step; the day already in force (`state().dataDate`) changes nothing and records none, even when it is a file's data date that is not the schedule's floor: no floor is set, so a host that wants one sets a different day first. */
  setDataDate(day: string | null): EditResult | undefined;
  /** Keep every task's dates as they are now as the baseline, replacing one already set. One undo step. With `records` (`[{ key: "t1", start: "2026-03-02", finish: "2026-03-06" }]`) make those dates by task key the baseline instead; a `YYYY-MM-DD` finish is the end of that day. A refusal sets nothing: `"badRecords"` (not an array, or one JSON cannot hold), or a record's reason with its `record` from 0: `"noKey"`, `"unknownKey"`, `"duplicateKey"`, `"badValue"` (its `member`) or `"finishBeforeStart"`. */
  setBaseline(records?: readonly { key: string; start: string; finish: string }[]): EditResult | undefined;
  /** Draw `records`, read as `setBaseline` reads them, as a second thin bar under each keyed task's comparison bar, in `--gantt-overlay`: a host's own dates, such as a leveled schedule's. `null` removes it. Not saved, not an edit and not in the undo list; no revision change; the bars do not move while a bar is dragged; a record `setBaseline` refuses is refused the same way and the overlay stays as it was. Opening a file drops it. */
  setOverlay(records: readonly { key: string; start: string; finish: string }[] | null): EditResult | undefined;
  /** Drop the baseline. `"noBaseline"` with none. One undo step. */
  clearBaseline(): EditResult | undefined;
  /** Give the tasks added since the baseline their current dates as baseline, keeping every other entry. `"noBaseline"` with none. One undo step. */
  addNewToBaseline(): EditResult | undefined;
  /**
   * Read `file`, an XER or MS Project XML file such as last month's, and make it the baseline: its tasks are matched to the open schedule's by ID.
   * Resolves `{ ok: true, fileOnly, hereOnly, fileOnlyLinks, hereOnlyLinks }`, `fileOnly` the tasks only the file has and `hereOnly` those only the open schedule has, one line each in that list's order (`A1020 Pour slab`: the ID, a space, the name),
   * and the two links lists the links only in each, matched by predecessor ID, successor ID and type (`A1010 → A1020 (FS)`),
   * the first 200 of each and then `and N more`; or a refusal that changes nothing: `"busy"`, or `"other"` with a sentence for a file that is empty, too big, not a
   * schedule file, or holds no task with an ID the open schedule has. One undo step: Ctrl+Z puts back the baseline that was in force, or none.
   */
  setBaselineFromFile(file: File): Promise<CompareResult> | undefined;
  /**
   * Ask what moves if each of `tasks` (up to 10, task numbers) slips a week, and change nothing: each gets a start-no-earlier-than of its early start plus 7 working days on a copy of the schedule, which is solved.
   * Answers `{ ok: true, finishWas, finishNow, moved }`, `moved` the tasks whose early start or finish would change with their task number, activity ID, name and dates before and after;
   * or `{ ok: false, reason, message }` for a summary, a started or unknown task, more than 10 tasks, and while a bar is pressed or dragged. No edit, no undo step, no event.
   */
  whatIf(tasks: readonly number[]): WhatIfResult | undefined;
  /**
   * Ask what moves if task `task` (a task number) takes `days` more working days (a whole number, 0 or more) of its calendar, and change nothing: a copy of the schedule with that longer duration is solved.
   * Answers as `whatIf` does, and refuses what it refuses; a task or `days` that is not a whole number is `"unknownTask"`. No edit, no undo step, no event.
   */
  whatIfLonger(task: number, days: number): WhatIfResult | undefined;
  /**
   * Ask why task `task` (a task number) starts when it does, and change nothing. Answers `{ ok: true, start, chain, capped }`, `chain` the tasks whose dependencies set its early start and theirs in turn, nearest first,
   * each with its task number, activity ID, name and total float; or `{ ok: false, reason, message }` for an unknown task, and while a bar is pressed or dragged. No edit, no undo step, no event.
   */
  explain(task: number): ExplainResult | undefined;
  /** Ask which tasks sit behind each `checks()` count that walks tasks, and change nothing: `{ ok: true, counts }`, each count `{ count, tasks }` (the true total and the first 50 tasks, with task number, activity ID and name), or `"busy"` while a bar is pressed. No edit, no undo step, no event. */
  health(): HealthResult | undefined;
  /** Ask which tasks are critical (zero total float, not finished) and which make up the longest path, and change nothing: `{ ok: true, critical, longest }`, each `{ count, tasks }` (the true total and the first 50 tasks, with task number, activity ID and name; no summary), or `"busy"` while a bar is pressed. The longest path can hold fewer tasks. No edit, no undo step, no event. */
  criticalPath(): CriticalPathResult | undefined;
  /**
   * Propose a change, held apart from the schedule while the chart draws a dashed ghost where each task it would move goes: `{ kind: "move", task, start }` (start no earlier than `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`),
   * `{ kind: "duration", task, days }` (working days of the task's calendar, 0 or more), `{ kind: "progress", task, actualStart }` (or `actualFinish`, `remaining` in working days, or `progress` as a percent from 0 to 100; one of the four; a person's Accept sets it as one undo step), `{ kind: "link", pred, succ, text }` (a link from task `pred` to task `succ`, `text` as `FS`, `SS+2d` or `FF-1d`) or `{ kind: "unlink", pred, succ }` (drop that link). Up to 10 changes are held; a second change of the same kind to a task replaces the first. Answers as `whatIf` does, with every held
   * change, or `{ ok: false, reason, message }` (`"badDate"` for a start that is not a date, `"finishBeforeStart"`, `"actualAfterDataDate"`, `"duration"` and `"badPercent"` for a progress value the schedule refuses, `"badLinkText"`, `"loop"`, `"alreadyLinked"` for a link the schedule already has, `"noLink"` for an unlink with none to drop, else `whatIf`'s reasons). The schedule, its undo history and its events stay as they were; an edit does not drop the proposal.
   */
  propose(change: ProposedChange): ProposeResult | undefined;
  /** Apply every held change as one undo step: `{ ok: true, changes }`, or `{ ok: false, reason }` with `"noProposal"`, `"busy"` (a cell editor open, a bar pressed or dragged) or `"other"` (a change the schedule now refuses; the proposal stays held). */
  acceptProposal(): AcceptResult | undefined;
  /** Drop every held change and change nothing else. True when a proposal was held. */
  rejectProposal(): boolean | undefined;
  /** Apply the one held change of `kind` to `task` as one undo step: `{ ok: true, changes: 1 }`, or `{ ok: false, reason }` as `acceptProposal` refuses, `"noProposal"` when that change is not held. The other changes stay held and their ghosts are solved again against the schedule it made. */
  acceptProposed(key: ProposedKey): AcceptResult | undefined;
  /** Drop the one held change of `kind` to `task` and change nothing else (no edit, no undo step); the other changes stay held and their ghosts are solved again without it. True when that change was held. */
  rejectProposed(key: ProposedKey): boolean | undefined;
  /**
   * The print setup now, as the JSON text `applyLayout` takes (`App.layout_json`): `name`, `paper` (`"letter"` by default), the row filter, the Lookahead window and the list columns the
   * chart shows, in its order (the Columns menu's hides and order, the Progress and Variance buttons: `printColumns`). It reads only. False, for a `paper` that is not `"letter"` or `"a4"`, or a `name` the app refuses.
   */
  layoutJson(name: string, paper?: PageSize): string | false | undefined;
  /**
   * Put a saved layout back (`App.apply_layout`): the row filter and the Lookahead window, then the columns (`applyColumns`). `{ ok: true, notApplied }`, `notApplied` one sentence for each part the schedule could not take, such as a filter that needs a
   * baseline it has not; a part not applied changes nothing. A refusal changes nothing: `"busy"`, or `"other"` with a sentence for text that is not a layout. The paper and the date window are not the chart's state: the page reads them from the text.
   */
  applyLayout(layout: string): ({ ok: true; notApplied: readonly string[] } | Refusal) | undefined;
}

/**
 * Build the commands over the chart's shell. `report` turns a thrown value into one line for the console; `busy` is `Gantt.busy()`.
 * @internal
 */
export function createCommands({ canvas, app, status, root }: Shell, report: (err: unknown) => string, busy: () => boolean): Commands {
  const select = (task: number | string): boolean => {
    const slot = typeof task === "string" ? (JSON.parse(app.task_slots([task])) as (number | null)[])[0] : task;
    return slot !== null && slot !== undefined && app.select_task(slot);
  };
  const history = (name: "undo" | "redo"): boolean => {
    try {
      return app[name]();
    } catch (err) {
      console.warn(`${name} failed: ${report(err)}`);
      return false;
    }
  };
  return {
    zoomIn: () => app.wheel(canvas.clientWidth / 2, 0, 0, -ZOOM_STEP_DY, true),
    zoomOut: () => app.wheel(canvas.clientWidth / 2, 0, 0, ZOOM_STEP_DY, true),
    fit: () => app.fit_project(),
    scrollTo(target) {
      if ("date" in target) return app.go_to_date(target.date);
      // Selecting a task brings its row and its bar into view, as a click on a row, a key or Find does.
      return select(target.task);
    },
    select,
    setFilter: (filter) => setFilter(app, status, filter),
    clearFilters: () => app.clear_filters(),
    setGroupBy: (codeType) => (codeType === null || typeof codeType === "string") && app.set_group_by(codeType ?? ""),
    undo: () => history("undo"),
    redo: () => history("redo"),
    layoutJson(name, paper = "letter") {
      try {
        return app.layout_json(name, paper, printColumns(root));
      } catch (err) {
        console.warn(`layoutJson failed: ${report(err)}`);
        return false;
      }
    },
    applyLayout(layout) {
      if (busy()) return refused("busy");
      try {
        const { not_applied: notApplied } = JSON.parse(app.apply_layout(layout)) as { not_applied: string[] };
        applyColumns(root, (JSON.parse(layout) as { columns: string[] | null }).columns);
        return { ok: true, notApplied };
      } catch (err) {
        return { ok: false, reason: "other", message: report(err) };
      }
    },
    ...createHostCommands(app, busy, report),
  };
}

/** Wrap `commands` so that each does nothing and returns undefined once `destroyed()` is true. Every method is wrapped, so a method added to `Commands` is too. */
export function guardCommands(commands: Commands, destroyed: () => boolean): Commands {
  const guarded: Record<string, unknown> = {};
  for (const [name, run] of Object.entries(commands) as [string, (...args: unknown[]) => unknown][]) {
    guarded[name] = (...args: unknown[]) => (destroyed() ? undefined : run(...args));
  }
  return guarded as unknown as Commands;
}
