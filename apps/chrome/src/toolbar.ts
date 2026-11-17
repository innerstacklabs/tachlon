import type { CheckKind, CompareResult, EditResult, LinkResult, RowResult, StatusKind, ViewState } from "gantt-core";
import { installBaselineButtons } from "./baseline-buttons.js";
import { hide } from "./code-filter.js";
import { installCompareFile } from "./compare-file.js";
import { type ChromeContext, type Piece, refocus } from "./parts.js";
import { installRowCountFit } from "./row-count-fit.js";

/** The Status choice's states as the row count words them: `Status = In progress`. */
const STATUS_NAMES: Record<StatusKind, string> = { "in-progress": "In progress", "not-started": "Not started", complete: "Complete" };
/** The Check choice's checks as the row count words them: `Check = Open starts`, the names the Checks window uses. */
const CHECK_NAMES: Record<CheckKind, string> = { "missing-logic": "Missing logic", "open-start": "Open starts", "open-end": "Open ends", "high-float": "High float", "high-duration": "High duration", "invalid-dates": "Invalid dates", "missing-resources": "Missing resources", missed: "Missed", "hard-constraints": "Hard constraints", "negative-float": "Negative float" };
/** The row filters that name themselves in the row count; the Status, activity code and Resource filters add their value. */
const FILTER_NAMES: Partial<Record<ViewState["filter"], string>> = {
  critical: "Critical only",
  longestPath: "Longest path",
  nearCritical: "Near critical",
  slipped: "Slipped",
  floatLost: "Float lost",
  joinedCritical: "Joined critical",
  leftCritical: "Left critical",
  moved: "Moved",
  startConstraint: "Start constraint",
};

/** The filters whose header count is of activities, not rows. */
const COUNTS_ACTIVITIES: ReadonlySet<ViewState["filter"]> = new Set(["critical", "nearCritical", "longestPath"]);

/** The toolbar's pieces: the chart leaves a slot for each. */
export interface ToolbarPieces {
  baseline: Piece;
  rows: Piece;
  zoom: Piece;
  readouts: Piece;
}

export interface Toolbar {
  /** The baseline buttons, the data date and the row count, from the view state (the `change` event). */
  sync: (state: ViewState) => void;
  /** A file was opened in place: show its data date and its activity code filter. */
  reset: () => void;
  /** The pieces went into the chart's slots (again, after a switch to WebGL2): the timeline names the row count as its description. */
  placed: () => void;
  /** Set `file` as the baseline (`commands.setBaselineFromFile`); a refusal shows in the chart's alert line and the chart takes focus back; it answers what the command answered. */
  compare: (file: File) => Promise<CompareResult | undefined>;
}

/**
 * The toolbar's controls beyond the Filters menu and the find box: the baseline buttons, Insert and Delete, the level buttons, the zoom
 * buttons, the data date and the row count. It drives the chart only through `commands` and shows what `state()` says. A refusal of a row
 * command or of the data date shows in the chart's alert line, a `busy` one is not shown (the press is simply ignored, as it always was).
 */
export function installToolbar(chrome: ChromeContext, pieces: ToolbarPieces): Toolbar {
  const { gantt, root } = chrome;
  const { commands } = gantt;
  const fmt = new Intl.NumberFormat("en-US");
  const click = (button: HTMLButtonElement, run: () => void): void => button.addEventListener("click", run);
  // The chart's alert line (`edit-error`, which its keys write to as well). The chrome clears a line it wrote when an edit or an open follows.
  let ownLine: string | null = null;
  const line = (): { box: HTMLElement; text: HTMLElement } | null => {
    const box = root.querySelector<HTMLElement>("[data-part=edit-error]");
    const text = root.querySelector<HTMLElement>("[data-part=edit-error-text]");
    return box === null || text === null ? null : { box, text };
  };
  const clearLine = (): void => {
    const shown = line();
    if (ownLine !== null && shown !== null && shown.text.textContent === ownLine) {
      shown.text.textContent = "";
      shown.box.hidden = true;
    }
    ownLine = null;
  };
  /** Show a refusal; false when there was none to show (`busy` is the press being ignored). */
  const refuse = (answer: EditResult | RowResult | LinkResult | undefined): boolean => {
    if (answer === undefined || answer.ok || answer.reason === "busy") return false;
    const shown = line();
    if (shown === null) console.warn(`gantt: ${answer.message}`);
    else {
      delete shown.box.dataset.kind;
      shown.text.textContent = answer.message;
      shown.box.hidden = false;
      ownLine = answer.message;
    }
    return true;
  };
  const baseline = installBaselineButtons(chrome, pieces.baseline);
  const compare = installCompareFile(chrome, refuse);
  chrome.hear("edit", clearLine);
  chrome.hear("open", clearLine);
  for (const [name, run] of [
    ["level-1", () => commands.collapseToLevel(1)],
    ["level-2", () => commands.collapseToLevel(2)],
    ["level-all", () => commands.expandAll()],
  ] as const) {
    click(pieces.rows.part(name), () => void run());
  }
  // Insert and delete act on the selected row, as the keys do. Delete removes a selected link first, as the Delete key does.
  const rowButton = (name: string, run: () => EditResult | RowResult | LinkResult | undefined): void =>
    click(pieces.rows.part(name), () => {
      refuse(run());
      refocus(root);
    });
  rowButton("insert-row", () => commands.insertRow());
  rowButton("delete-row", () => {
    const link = commands.deleteLink();
    return link?.ok === false && link.reason === "noSelection" ? commands.deleteRow() : link;
  });
  click(pieces.zoom.part("zoom-in"), () => commands.zoomIn());
  click(pieces.zoom.part("zoom-out"), () => commands.zoomOut());

  // The data date (gs#703). The span on the right says it. The toolbar has no room for a second always-visible control (at 1280 px the
  // find box is already down to a few pixels beside a file name), so the box that moves it takes the span's place: a schedule with no
  // data date shows the box, one with a data date shows the span, and clicking the span (or Enter on it) swaps the box in. The box takes a
  // day written YYYY-MM-DD; an empty box clears the date; Enter applies it through `commands.setDataDate`; Escape or leaving the box puts the
  // shown date back; a date the chart refuses shows in the alert line, with the box marked invalid.
  const dataDateText = pieces.readouts.part<HTMLSpanElement>("data-date");
  const dateInput = pieces.readouts.part<HTMLInputElement>("data-date-input");
  const rowCount = pieces.readouts.part<HTMLSpanElement>("row-count");
  let shownDate: string | null = null;
  let editing = false;
  const shownDay = (): string => (shownDate ?? "").slice(0, 10);
  const layoutDataDate = (): void => {
    const none = shownDate === "";
    hide(dataDateText, none || editing);
    hide(dateInput, !none && !editing);
  };
  const showDataDate = (state: ViewState): void => {
    const date = state.dataDateTime ?? "";
    if (date === shownDate) return;
    shownDate = date;
    dataDateText.textContent = date === "" ? "" : `Data date ${date}`;
    dataDateText.title = date === "" ? "" : `${dataDateText.textContent}. Click to change it; press T to go to it.`;
    if (!editing) dateInput.value = shownDay();
    layoutDataDate();
  };
  const clearRefusal = (): void => {
    dateInput.removeAttribute("aria-invalid");
    clearLine();
  };
  const openDateBox = (): void => {
    if (editing) return;
    editing = true;
    dateInput.value = shownDay();
    layoutDataDate();
    dateInput.focus();
    dateInput.select();
  };
  /** Put the shown date back and the span in the box's place; `refocus`: the chart takes the keys again (not when focus went elsewhere). */
  const closeDateBox = (again: boolean): void => {
    editing = false;
    dateInput.value = shownDay();
    clearRefusal();
    layoutDataDate();
    if (again) refocus(root);
  };
  const applyDataDate = (): void => {
    const text = dateInput.value.trim();
    // The box shows the day and a file's data date may carry a time: text that is still the shown day changes nothing.
    if (text === shownDay()) return closeDateBox(true);
    const answer = commands.setDataDate(text === "" ? null : text);
    if (refuse(answer)) {
      dateInput.setAttribute("aria-invalid", "true");
      return;
    }
    // A press the chart ignored (busy) leaves the box open; an applied date is shown by the next `change`, and now.
    if (answer?.ok === true) {
      showDataDate(gantt.state());
      closeDateBox(true);
    }
  };
  dataDateText.setAttribute("role", "button");
  dataDateText.tabIndex = 0;
  dataDateText.addEventListener("click", openDateBox);
  dataDateText.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      // The key is handled here; bubbling it would let the chart's cursor open the selected cell's editor and steal the focus.
      e.stopPropagation();
      openDateBox();
    }
  });
  dateInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      applyDataDate();
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeDateBox(true);
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      // A day (a week with Shift) off the typed date, in the box's text only: Enter is still the one act that records it.
      e.preventDefault();
      const typed = Date.parse(`${dateInput.value.trim()}T00:00:00Z`);
      if (Number.isNaN(typed)) return;
      const days = (e.shiftKey ? 7 : 1) * (e.key === "ArrowUp" ? 1 : -1);
      dateInput.value = new Date(typed + days * 86_400_000).toISOString().slice(0, 10);
      clearRefusal();
    }
  });
  dateInput.addEventListener("input", clearRefusal);
  dateInput.addEventListener("blur", () => closeDateBox(false));

  const fit = installRowCountFit(chrome, rowCount);
  const showRowCount = (state: ViewState): void => {
    const filterLabel =
      [
        FILTER_NAMES[state.filter] ?? null,
        state.filter === "code" && state.codeFilter !== null ? `${state.codeFilter.type} = ${state.codeFilter.value}` : null,
        state.filter === "resource" && state.resourceFilter !== null ? `Resource = ${state.resourceFilter}` : null,
        state.filter === "check" && state.checkFilter !== null ? `Check = ${CHECK_NAMES[state.checkFilter]}` : null,
        state.filter === "status" && state.statusFilter !== null ? `Status = ${STATUS_NAMES[state.statusFilter]}` : null,
        state.lookahead.on ? "Lookahead" : null,
      ]
        .filter((label) => label !== null)
        .join(" + ") || null;
    // Slipped alone counts the tasks that slipped (the baseline summary's number), not the rows, which keep their summaries, and says it in as few letters as that takes: "46 slipped" (gs#858).
    const tasks = state.filter === "slipped" && filterLabel === "Slipped" ? state.baseline?.slipped : undefined;
    // Critical only, Near critical and Longest path count the activities they keep too, not the rows, which also hold the summaries above them.
    const activities = COUNTS_ACTIVITIES.has(state.filter) ? state.activityCount : null;
    const rows = fmt.format(state.rowCount);
    const text =
      tasks !== undefined
        ? `${fmt.format(tasks)} slipped`
        : filterLabel && activities !== null
          ? `${filterLabel} · ${fmt.format(activities)} of ${fmt.format(state.taskCount)} activities`
          : filterLabel
            ? `${filterLabel} · ${rows} of ${fmt.format(state.taskCount)} rows`
            : `${rows} rows`;
    if (rowCount.textContent !== text) rowCount.textContent = text;
    // "46 slipped" is a filter count too, so it never gives way.
    fit.fit(filterLabel === null && tasks === undefined);
  };
  const sync = (state: ViewState): void => {
    baseline(state);
    showDataDate(state);
    showRowCount(state);
  };
  // The timeline is described by the row count the chrome placed: the id the chart gave the slot, so the reference names this chart's element.
  const placed = (): void => {
    fit.watch();
    root.querySelector("[data-part=timeline]")?.setAttribute("aria-describedby", rowCount.id);
  };
  return { sync, reset: () => sync(gantt.state()), placed, compare };
}
