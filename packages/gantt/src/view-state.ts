// The view state a host's toolbar shows (gs#1194): `Gantt.state()` reads it and `Gantt.on("change")` hears it change. It is
// built from the `Status` the loop already reads once a frame. `read` compares the few fields it carries with the last state
// it built and returns that same object when none moved, so a frame where nothing a toolbar shows changed allocates nothing
// and the change event, which fires on a new object, stays quiet. The status and activity code filters are read from the app
// only when `Status.stamp`, `Status.epoch` or the filter's name moved (every filter change moves the stamp). The data date and
// the baseline (its slipped and added counts taken from the status, its day text from the app) are read only when `Status.epoch` or `Status.stamp` moved (each change
// of them moves the stamp) and no bar is being dragged.
import type { PublicApp, Status } from "./api.js";
import { publicFilterName, type RowFilterName } from "./filter-names.js";

export type { RowFilterName } from "./filter-names.js";

/** The time axis's unit at the current zoom. */
export type ZoomLevel = "hour" | "day" | "week" | "month" | "year";

/** The task states the Status filter keeps. */
export type StatusKind = "in-progress" | "not-started" | "complete";

/** The ten schedule checks whose tasks the Check filter keeps. */
export type CheckKind = "missing-logic" | "open-start" | "open-end" | "high-float" | "high-duration" | "invalid-dates" | "missing-resources" | "missed" | "hard-constraints" | "negative-float";

/** An activity code type and one of its values, as `filterInputs().activityCodes` lists them. */
export interface CodeValue {
  type: string;
  value: string;
}

/** What `Gantt.state()` returns and the `change` event carries. Frozen: a host reads it, the chart never changes it. */
export interface ViewState {
  /** The row filter that is on, `"off"` for none. */
  filter: RowFilterName;
  /** The Status filter's state while `filter` is `"status"`, else null. */
  statusFilter: StatusKind | null;
  /** The Check filter's check while `filter` is `"check"`, else null. */
  checkFilter: CheckKind | null;
  /** The activity code filter while `filter` is `"code"`, else null. */
  codeFilter: CodeValue | null;
  /** The Resource filter's resource name while `filter` is `"resource"`, else null. */
  resourceFilter: string | null;
  /** The activity code type the rows are grouped by, or null while they are not. It is view state: a saved document does not carry it. */
  groupBy: string | null;
  /** The lookahead window, which combines with the row filter. While `on` is false, `start` and `weeks` are what turning it on would use. */
  lookahead: { on: boolean; start: string; weeks: number };
  /** The rows the filters and the collapsed summaries leave. */
  rowCount: number;
  /** The activities (leaf tasks) the row filter and the lookahead window keep, or null while neither is on. */
  activityCount: number | null;
  /** While Longest path is on, what the line at the chart's bottom left names: the path's first and last activity, its activities and the calendar days from the first start to the project finish; else null. */
  path: Readonly<{ first: string; last: string; count: number; days: number }> | null;
  /** The live tasks of the schedule, rows or not. */
  taskCount: number;
  /** The time axis's unit at this zoom. */
  zoomLevel: ZoomLevel;
  /** The selected task with the counts of its driving predecessors and driven successors, or null. */
  selection: { task: number; backCount: number; forwardCount: number } | null;
  /**
   * The last edit's report of what it moved, or null: `others` is the number of tasks besides the edited one whose early start or
   * finish changed, `finishDays` the calendar days the project finish moved (negative when earlier). `id` tells one report from the
   * next and means nothing else. A host words the report itself; the chart's own sentence is not part of the API.
   */
  moved: { id: number; others: number; finishDays: number } | null;
  /** The data date, the day written `YYYY-MM-DD` (a file's time of day is not in it), or null with none. `commands.setDataDate` takes the same form. */
  dataDate: string | null;
  /** The data date's instant as the toolbar shows it, `YYYY-MM-DD HH:MM` (`2024-01-10 08:00`), or null with none. */
  dataDateTime: string | null;
  /**
   * The baseline, or null with none: `rows` is the number of tasks it holds, `dataDate` the data date's day when it was set (`YYYY-MM-DD`),
   * null if there was none, `dayText` that same day as the chart writes it (`14 Oct`, the form of the Baseline button's title), null if
   * there was none, `slipped` the tasks whose early finish is now later than the baseline's, and `added` the live tasks it has no entry for
   * (inserted since it was set), which "Add new" gives their current dates.
   */
  baseline: { rows: number; dataDate: string | null; dayText: string | null; slipped: number; added: number } | null;
  /** The backend that is drawing. */
  backend: "WebGPU" | "WebGL2";
  /** The name of the file opened in place (`one-project.xer`), or null for a sample or a schedule loaded with `load({ document })`. A mount with a File and `load({ file })` give that file's name. */
  fileName: string | null;
  /** True after an edit, undo or redo since the last open or save: the mark beside the file name. */
  unsaved: boolean;
  /** The import notes the panel lists: always what it shows (after a mount, an open, a load and a restart on WebGL2), the reader's lines without the ones `expectedNotes` names. Frozen. */
  importNotes: readonly string[];
  /** The names of the members of your rows that the chart has no field for, for tasks and for links: the chart keeps them with the row and gives them back in a create's `set` and a delete's `was` (`fields` is the map of your own names). Frozen; the same object until a new name is seen. */
  customFields: Readonly<{ tasks: readonly string[]; links: readonly string[] }>;
}

/** What the chart knows of the open file; the reader compares these with the last state it built. */
export interface OpenFacts {
  fileName: string | null;
  unsaved: boolean;
  importNotes: readonly string[];
  customFields: Readonly<{ tasks: readonly string[]; links: readonly string[] }>;
}

/** What `Gantt.filterInputs()` returns: what the filter controls offer. It changes with an open (`open`), an edit (`edit`) and a baseline set, cleared or added to, which fires no `edit`: a `change` whose `baseline` is a new object. */
export interface FilterInputs {
  /** The activity code types the tasks carry, each with its values, by name: `[type, values]`. Empty for a file with none. */
  activityCodes: [string, string[]][];
  /** The resource names the tasks carry, by name, each once: what the Resource filter offers. Empty for a file that names none. */
  resources: string[];
  /** Whether any task has an activity ID (the table then shows the ID column). */
  hasCodes: boolean;
  /** Whether the baseline holds float, which the `"floatLost"` filter compares with: false with no baseline or one read from a file. */
  baselineHasFloat: boolean;
}

/**
 * Read the filter inputs from the app. Not per frame: a host calls it.
 * @internal
 */
export function readFilterInputs(app: PublicApp): FilterInputs {
  return {
    activityCodes: JSON.parse(app.activity_codes()) as [string, string[]][],
    resources: JSON.parse(app.resource_names()) as string[],
    hasCodes: app.has_codes(),
    baselineHasFloat: app.baseline_has_float(),
  };
}

export interface ViewStateReader {
  /** The view state `s` shows: the object `read` returned last when nothing it carries moved, else a new frozen one. */
  read(s: Status): ViewState;
}

/**
 * `open` is told the status first, so the unsaved mark it reads is the one this frame shows.
 * @internal
 */
export function createViewState(app: PublicApp, open: (s: Status) => OpenFacts): ViewStateReader { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const backend = app.backend() === "WebGPU" ? "WebGPU" : "WebGL2";
  let last: ViewState | null = null;
  // The status and code filter values, read again only when one of these moved.
  let readAt: [epoch: number, stamp: number, filter: string] = [-1, -1, ""];
  let statusFilter: StatusKind | null = null;
  let checkFilter: CheckKind | null = null;
  let codePair = "null";
  let codeFilter: CodeValue | null = null;
  let resourceFilter: string | null = null;
  let groupBy: string | null = null;
  const filterValues = (s: Status): void => {
    if (readAt[0] === s.epoch && readAt[1] === s.stamp && readAt[2] === s.filter) return;
    readAt = [s.epoch, s.stamp, s.filter];
    statusFilter = s.filter === "status" ? ((app.status_filter() || null) as StatusKind | null) : null;
    checkFilter = s.filter === "check" ? ((app.check_filter() || null) as CheckKind | null) : null;
    resourceFilter = s.filter === "resource" ? app.resource_filter() || null : null;
    groupBy = app.group_by() || null;
    // The pair as it was set (`App.activity_code_filter_pair`), not the header label, which would need splitting.
    const pair = s.filter === "code" ? app.activity_code_filter_pair() : "null";
    if (pair !== codePair) {
      codePair = pair;
      const parsed = JSON.parse(pair) as [string, string] | null;
      codeFilter = parsed === null ? null : { type: parsed[0], value: parsed[1] };
    }
  };
  // The data date and the baseline, read again only when one of these moved.
  let factsAt: [epoch: number, stamp: number] = [-1, -1];
  let dataDate: string | null = null;
  let dataDateTime: string | null = null;
  let baseline: ViewState["baseline"] = null;
  let float = false;
  const facts = (s: Status): void => {
    // A drag moves the stamp each frame and can change neither; its release is read.
    if ((factsAt[0] === s.epoch && factsAt[1] === s.stamp) || s.drag?.active === true) return;
    factsAt = [s.epoch, s.stamp];
    dataDateTime = app.data_date_text() || null;
    dataDate = dataDateTime?.slice(0, 10) ?? null;
    const rows = app.baseline_rows();
    const day = rows > 0 ? app.baseline_day() || null : null;
    // A baseline set over one read from a file can keep its rows and day and gain float, which `filterInputs` reads:
    // a new object then, so `change` fires and the host reads the filter inputs again.
    const hasFloat = rows > 0 && app.baseline_has_float();
    // The count comes with the status, which carries it each frame; every change of it moves the stamp.
    const slipped = s.baseline?.slipped ?? 0;
    const added = s.baseline?.added ?? 0;
    // `dayText` is the day `day` names, so one of them moving moves the other.
    const dayText = rows > 0 ? app.baseline_data_date() || null : null;
    if (rows === 0) baseline = null;
    else if (
      baseline?.rows !== rows ||
      baseline.dataDate !== day ||
      baseline.dayText !== dayText ||
      baseline.slipped !== slipped ||
      baseline.added !== added ||
      hasFloat !== float
    )
      baseline = Object.freeze({ rows, dataDate: day, dayText, slipped, added });
    float = hasFloat;
  };
  const same = (v: ViewState, s: Status, o: OpenFacts): boolean =>
    v.fileName === o.fileName &&
    v.unsaved === o.unsaved &&
    v.importNotes === o.importNotes &&
    v.customFields === o.customFields &&
    v.dataDate === dataDate &&
    v.dataDateTime === dataDateTime &&
    v.baseline === baseline &&
    v.filter === publicFilterName(s.filter) &&
    v.statusFilter === statusFilter &&
    v.checkFilter === checkFilter &&
    v.codeFilter === codeFilter &&
    v.resourceFilter === resourceFilter &&
    v.groupBy === groupBy &&
    v.lookahead.on === s.lookahead.on &&
    v.lookahead.start === s.lookahead.start &&
    v.lookahead.weeks === s.lookahead.weeks &&
    v.rowCount === s.rowCount &&
    v.activityCount === s.keptLeaves &&
    v.path?.first === s.path?.first && v.path?.last === s.path?.last &&
    v.path?.count === s.path?.count && v.path?.days === s.path?.days &&
    v.taskCount === s.n &&
    v.zoomLevel === s.level &&
    v.selection?.task === s.selected?.task &&
    v.selection?.backCount === s.selected?.backCount &&
    v.selection?.forwardCount === s.selected?.forwardCount &&
    // The report, not its fade: `alpha` moves every frame while the note fades.
    v.moved?.id === s.moved?.seq &&
    v.moved?.others === s.moved?.others &&
    v.moved?.finishDays === s.moved?.finish_days;
  return {
    read(s) {
      filterValues(s);
      facts(s);
      const o = open(s);
      if (last !== null && same(last, s, o)) return last;
      const { selected, moved } = s;
      last = Object.freeze({
        filter: publicFilterName(s.filter),
        statusFilter,
        checkFilter,
        codeFilter: codeFilter === null ? null : Object.freeze({ ...codeFilter }),
        resourceFilter,
        groupBy,
        lookahead: Object.freeze({ ...s.lookahead }),
        rowCount: s.rowCount,
        activityCount: s.keptLeaves,
        path: s.path === null ? null : Object.freeze({ ...s.path }),
        taskCount: s.n,
        zoomLevel: s.level,
        selection: selected === null ? null : Object.freeze({ task: selected.task, backCount: selected.backCount, forwardCount: selected.forwardCount }),
        moved: moved === null ? null : Object.freeze({ id: moved.seq, others: moved.others, finishDays: moved.finish_days }),
        dataDate,
        dataDateTime,
        baseline,
        backend,
        fileName: o.fileName,
        unsaved: o.unsaved,
        importNotes: o.importNotes,
        customFields: o.customFields,
      });
      // The frozen copy of the code filter is what the next comparison sees.
      codeFilter = last.codeFilter;
      return last;
    },
  };
}
