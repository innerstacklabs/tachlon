import { type HeaderRow, type Status, type TableRow } from "./api.js";
import { type CellEdit } from "./cell-edit.js";
import { ORDER_FLAG } from "./column-order.js";
import { HIDE_FLAGS, isHidden } from "./columns-menu.js";
import { type Cursor } from "./cursor.js";
import { element } from "./dom.js";
import { nameOf } from "./field-names.js";
import { frozenEdge } from "./frozen-edge.js";
import { installRowPosition } from "./row-position.js";
import { type Shell, watching } from "./shell.js";
import { trackChanged } from "./table-changed.js";
import { buildRow } from "./table-row.js";
import { fitTaskWidth, measureTaskNeed } from "./task-width.js";

/** Rows kept in the DOM above and below the visible ones. */
const TABLE_OVERSCAN = 6;

export interface Table {
  /** Scroll, select, hover and draw the rows for this status. */
  sync: (s: Status) => void;
  /** Draw the rows again at the next frame. */
  invalidate: () => void;
  /** Show or hide the four progress columns, and draw the rows again. */
  showProgress: (on: boolean) => void;
  /** A new schedule was opened: forget the scroll, the selection and the hover. */
  reset: () => void;
  /** Mark these task slots as pending in place of the ones marked before, and draw the rows again. */
  setPending: (slots: readonly number[]) => void;
  /** An `update` wrote these task slots: they stop being pending, and the next redraw highlights the cells whose text changed. */
  wrote: (slots: readonly number[]) => void;
}

/**
 * Task table (plan D3): DOM rows for the visible window only, inside a
 * spacer as tall as all the rows so the browser scrolls it natively. Its
 * scrollTop and the canvas's scroll_y are one value, shared both ways.
 */
export function installTable( // oxlint-disable-line max-lines-per-function -- existing, over cap
  shell: Shell,
  edit: CellEdit,
  hasCodes: () => boolean,
  s0: Status,
  cursor: Cursor,
  progressOn: () => boolean,
): Table {
  const { app, root, fmt } = shell;
  const tableBody = element<HTMLDivElement>(root, "table-body");
  const tableRows = element<HTMLDivElement>(root, "table-rows");
  tableRows.style.height = `${s0.rowCount * s0.rowHeight}px`;
  let spacerRows = s0.rowCount;
  const tableEl = element<HTMLDivElement>(root, "table");
  const tableHead = element<HTMLElement>(root, "table-head");
  const headCols = tableHead.querySelector<HTMLElement>(".cols");
  // The columns can need more room than the pane: the body scrolls sideways and the head follows. data-scrolled tells the stylesheet to
  // draw the edge of the frozen columns (Task, and ID and WBS when shown). Only a change of the offset writes to the page.
  // The body keeps a vertical scrollbar; the head has none, so it pads its end side (the left in a right-to-left chart) by that width and the two stay as wide as each other.
  // A classic sideways scrollbar takes its height from the body, and the canvas's last scroll stops that much short of the last row (gs#811):
  // tell it the height. Asked on a resize, a column change and an open (a new file starts the canvas with none), never per frame.
  let scrollSlack = -1;
  const syncScrollSlack = (): void => {
    const bar = tableBody.offsetHeight - tableBody.clientHeight;
    if (bar === scrollSlack) return;
    scrollSlack = bar;
    app.set_scroll_slack(bar);
  };
  watching(shell, new ResizeObserver(() => {
    tableHead.style.paddingInlineEnd = `${tableBody.offsetWidth - tableBody.clientWidth}px`;
    syncScrollSlack();
  })).observe(tableBody);
  // Task gives room so the pane edge falls between columns, and takes what the file's longest name needs (task-width.ts; gs#851, gs#959).
  let taskNeed = 0; // measured once per opened file, below
  // With Progress on the CSS puts % complete and the other progress cells after Task, so the head is read in the order it shows (gs#974).
  const shown = (): HTMLElement | null => {
    if (headCols === null || (tableEl.dataset["progress"] !== "true" && !tableEl.hasAttribute("data-resources") && !tableEl.hasAttribute("data-pred") && ![...HIDE_FLAGS, ORDER_FLAG].some((flag) => tableEl.hasAttribute(flag)))) return headCols;
    const order = (cell: Element): number => Number(getComputedStyle(cell).order) || 0;
    // A column the Columns menu hid has no track (table.css), so its head is not one of the columns the pane holds.
    const children = [...headCols.children].filter((cell) => !isHidden(tableEl, cell.classList[0] ?? "")).sort((a, b) => order(a) - order(b));
    return new Proxy(headCols, { get: (head, key) => (key === "children" ? children : Reflect.get(head, key) instanceof Function ? (Reflect.get(head, key) as Function).bind(head) : Reflect.get(head, key)) });
  };
  const fitTask = (): void => fitTaskWidth(root, tableBody, shown(), taskNeed);
  watching(shell, new ResizeObserver(fitTask)).observe(tableBody);
  // The toolbar adds the ID head and sets data-ids after an open (showFileFacts runs after reset), so the fit made in reset missed that column
  // and left the last date column past the pane (gs#880). Fit again when a head cell comes or goes or a column flag changes; a mutation
  // observer runs at a change of columns, never per frame.
  if (headCols) {
    const refit = watching(shell, new MutationObserver(fitTask));
    refit.observe(headCols, { childList: true });
    refit.observe(tableEl, { attributes: true, attributeFilter: ["data-ids", "data-wbs", "data-pred", "data-progress", "data-variance", "data-variance-hidden", ...HIDE_FLAGS, ORDER_FLAG] });
  }
  const hideCutColumn = frozenEdge(tableEl, headCols);
  // data-more is set while more columns lie to the right of the pane (gs#974): the stylesheet draws a fade there with Progress on. It is read at a
  // scroll of the table, a resize and a change of columns, never per frame of the chart.
  const markMore = (): void => {
    const more = Math.abs(tableBody.scrollLeft) < tableBody.scrollWidth - tableBody.clientWidth - 1;
    if (more !== (tableEl.dataset.more === "true")) {
      if (more) tableEl.dataset.more = "true";
      else delete tableEl.dataset.more;
    }
  };
  watching(shell, new ResizeObserver(markMore)).observe(tableBody);
  watching(shell, new MutationObserver(markMore)).observe(tableEl, { attributes: true, attributeFilter: ["data-ids", "data-wbs", "data-pred", "data-progress", "data-variance", "data-variance-hidden", ...HIDE_FLAGS, ORDER_FLAG] });
  tableBody.addEventListener("scroll", () => {
    markMore();
    if (tableHead.scrollLeft !== tableBody.scrollLeft) tableHead.scrollLeft = tableBody.scrollLeft;
    if ((tableBody.scrollLeft !== 0) !== (tableEl.dataset.scrolled === "true")) {
      if (tableBody.scrollLeft !== 0) tableEl.dataset.scrolled = "true";
      else delete tableEl.dataset.scrolled;
    }
    hideCutColumn();
  });
  // The ID and WBS tracks are as wide as the file's longest ID and WBS code (text, padding, a floor, and a cap; all tokens). Each row is its own
  // grid, so a track cannot size itself to the widest value: read the two codes once per opened file and set the widths on the root in px.
  // The root grid and the divider read --gantt-id-col as px, so it is measured here, in the table's font, rather than written in ch.
  const sizeCodeTracks = (): void => {
    const [id = "", wbs = ""] = app.longest_codes() as string[];
    const probe = document.createElement("span");
    probe.style.cssText = "position:absolute;visibility:hidden;white-space:nowrap";
    tableEl.append(probe);
    const track = (text: string, cap: string): string => {
      probe.style.width = "";
      probe.textContent = text;
      const textPx = probe.getBoundingClientRect().width;
      probe.style.width = `min(max(calc(${textPx}px + var(--gantt-code-pad)), var(--gantt-code-min)), var(${cap}))`;
      return `${probe.getBoundingClientRect().width}px`;
    };
    root.style.setProperty("--gantt-id-col", track(id, "--gantt-id-max"));
    root.style.setProperty("--gantt-wbs-col", track(wbs, "--gantt-wbs-max"));
    probe.remove();
  };
  sizeCodeTracks();
  taskNeed = measureTaskNeed(tableEl, app.longest_name() as string[]);
  // The WBS column exists only for a file with WBS codes; asked once per opened file (the scan is not for a frame).
  let hasWbs = false;
  const showWbsColumn = (): void => {
    hasWbs = app.has_wbs_codes();
    headCols?.querySelector(".wbs")?.remove();
    if (hasWbs) {
      const head = document.createElement("span");
      head.className = "wbs";
      head.textContent = nameOf(root, "wbs");
      const code = headCols?.querySelector(".code");
      if (code) code.after(head);
      else headCols?.prepend(head);
      tableEl.dataset.wbs = "true";
    } else {
      delete tableEl.dataset.wbs;
    }
  };
  showWbsColumn();
  // The Predecessors column shows for exactly the files the ID column does, since a typed list names Activity IDs; asked once per opened file.
  let hasPreds = false;
  const showPredColumn = (): void => {
    hasPreds = app.has_codes();
    headCols?.querySelector(".pred")?.remove();
    tableEl.toggleAttribute("data-pred", hasPreds);
    if (hasPreds) headCols?.querySelector(".float")?.after(Object.assign(document.createElement("span"), { className: "pred", textContent: nameOf(root, "predecessors") }));
  };
  showPredColumn();
  let hasResources = false;
  const showResourcesColumn = (): void => {
    hasResources = app.has_resources();
    headCols?.querySelector(".resources")?.remove();
    tableEl.toggleAttribute("data-resources", hasResources);
    if (hasResources) headCols?.append(Object.assign(document.createElement("span"), { className: "resources", textContent: nameOf(root, "resources") }));
  };
  showResourcesColumn();
  // The four progress columns show when the Progress toggle is on: it starts on for a file that carries progress, asked once per opened file, like the WBS column.
  let hasProgress = false;
  const PROGRESS_HEADS: [string, string][] = [
    ["actual-start", "actual-start"],
    ["actual-finish", "actual-finish"],
    ["remaining", "remaining"],
    ["percent", "percent-head"],
  ];
  const showProgressColumns = (on: boolean): void => {
    hasProgress = on;
    for (const [cls] of PROGRESS_HEADS) headCols?.querySelector(`.${cls}`)?.remove();
    if (hasProgress) {
      for (const [cls, key] of PROGRESS_HEADS) {
        const head = document.createElement("span");
        head.className = cls;
        head.textContent = nameOf(root, key);
        headCols?.append(head);
      }
      tableEl.dataset.progress = "true";
    } else {
      delete tableEl.dataset.progress;
    }
  };
  showProgressColumns(progressOn());
  fitTask();
  syncScrollSlack();
  // The table's row and head heights are the canvas's, so the rows line up.
  root.style.setProperty("--gantt-row-h", `${s0.rowHeight}px`);
  root.style.setProperty("--gantt-axis-h", `${s0.axisHeight}px`);
  const position = installRowPosition(root, fmt);
  let canvasScrollY = 0; // the canvas's scroll_y at the last frame
  let drawn: { first: number; last: number; stamp: number; variance: string } = { first: -1, last: -1, stamp: -1, variance: "off" };
  let selectedTask: number | null = null;
  // The host's pending marks, by task slot, and the schedule (`Status.epoch`) they were made for: the first frame of another schedule drops them,
  // so marks made right after a `load` (before that frame) stay.
  let pending: ReadonlySet<number> = new Set();
  let pendingEpoch = s0.epoch;
  // After `wrote` the next redraw marks each cell whose text differs from the one shown (data-changed, table-changed.ts).
  const changed = trackChanged(tableRows, (undo) => shell.disposers.push(undo));
  let extentRow: number | null = null; // the range's moving end at the last frame
  tableBody.addEventListener("scroll", () => {
    // Within a pixel of the canvas's own value is the echo of our write below.
    if (Math.abs(tableBody.scrollTop - canvasScrollY) >= 1) app.set_scroll_y(tableBody.scrollTop);
    position.show(tableBody.scrollTop, s0.rowHeight, spacerRows);
  });
  // Mark a table row with data-<attr>="true"; the stylesheet draws the mark
  // with --gantt-sel / --gantt-hov, the tokens the canvas fills the same row
  // with. Rows are replaced on a redraw, so the mark is applied again then;
  // otherwise only when the marked row changes.
  const rowMark = (attr: "selected" | "hover") => {
    let markedRow: number | null = null;
    let markedElement: HTMLElement | null = null;
    return (next: number | null, redrawn: boolean): void => {
      if (!redrawn && next === markedRow) return;
      if (markedElement) delete markedElement.dataset[attr];
      markedRow = next;
      markedElement = next === null ? null : tableRows.querySelector<HTMLElement>(`.row[data-row="${next}"]`);
      if (markedElement) markedElement.dataset[attr] = "true";
    };
  };
  const markSelected = rowMark("selected");
  const markHover = rowMark("hover");
  // The selected row and the row the canvas draws as hovered, whichever
  // pointer put it there (the canvas's or this table's).
  const markRows = (s: Status, redrawn: boolean): void => {
    markSelected(s.selected?.row ?? null, redrawn);
    markHover(s.hoverRow, redrawn);
    cursor.mark(s, redrawn);
  };
  // The table's pointer sets the canvas's hover row. The canvas gives the row
  // back in the status, so both halves of the row are filled from one value.
  // The table pointer's offset inside the table body, kept so a stationary
  // pointer follows the rows as they scroll past it (the canvas hover row is
  // an absolute row; the pointer is not).
  let tableHoverOffset: number | null = null;
  let tableHoverRow: number | undefined;
  const setTableHover = (row: number | undefined): void => {
    if (row === tableHoverRow) return;
    tableHoverRow = row;
    app.set_hover_row(row);
  };
  tableRows.addEventListener("pointermove", (e) => {
    if (e.pointerType === "touch" || !(e.target instanceof Element)) return;
    const row = e.target.closest<HTMLElement>(".row");
    tableHoverOffset = row === null ? null : e.clientY - tableBody.getBoundingClientRect().top;
    setTableHover(row === null ? undefined : Number(row.dataset.row));
  });
  tableBody.addEventListener("pointerleave", () => {
    tableHoverOffset = null;
    setTableHover(undefined);
  });
  // Re-aim the stationary table pointer after a scroll or a change in the row count.
  const followTablePointer = (s: Status): void => {
    if (tableHoverOffset === null) return;
    const row = Math.floor((s.scrollY + tableHoverOffset) / s.rowHeight);
    setTableHover(row >= 0 && row < s.rowCount ? row : undefined);
  };
  tableRows.addEventListener("click", (e) => {
    if (edit.active() || !(e.target instanceof Element)) return;
    const disclosure = e.target.closest<HTMLElement>(".disclosure");
    if (disclosure === null) return;
    const row = disclosure.closest<HTMLDivElement>(".row");
    const task = Number(row?.dataset.task);
    // A group header has no task: its chevron closes or opens the group, named by its value (none for the leaves with no value).
    if (row?.dataset.kind === "3" || row?.dataset.kind === "4") app.toggle_group(row.dataset.group ?? null);
    else if (row === null || !Number.isInteger(task)) return;
    else app.toggle_task(task);
    e.preventDefault();
    e.stopPropagation();
  });
  const syncTable = (s: Status): void => { // oxlint-disable-line max-lines-per-function -- existing, over cap
    canvasScrollY = s.scrollY;
    if (s.epoch !== pendingEpoch) {
      pending = new Set();
      changed.drop();
      pendingEpoch = s.epoch;
    }
    if (s.rowCount !== spacerRows) {
      tableRows.style.height = `${s.rowCount * s.rowHeight}px`;
      spacerRows = s.rowCount;
    }
    if (Math.abs(tableBody.scrollTop - s.scrollY) >= 1) tableBody.scrollTop = s.scrollY;
    const nextSelectedTask = s.selected?.task ?? null;
    // The row to bring into view: the selected row, or the range's moving end (gs#423) while there is a range.
    const nextExtent = s.range?.[2] ?? null;
    if (nextSelectedTask !== selectedTask || nextExtent !== extentRow) {
      selectedTask = nextSelectedTask;
      extentRow = nextExtent;
      if (s.selected) {
        const top = (nextExtent ?? s.selected.row) * s.rowHeight;
        const bottom = top + s.rowHeight;
        if (top < tableBody.scrollTop || bottom > tableBody.scrollTop + tableBody.clientHeight) {
          tableBody.scrollTop = Math.max(0, top - (tableBody.clientHeight - s.rowHeight) / 2);
        }
      }
    }
    // Hover and selection marks keep following the canvas while a cell is edited; the rows are not rebuilt then.
    followTablePointer(s);
    if (edit.active()) {
      markRows(s, false);
      return;
    }
    // The overscan is capped at half the visible rows, so the DOM never holds
    // more than twice what is visible, however small the view.
    const visible = s.rows[1] - s.rows[0];
    const overscan = Math.min(TABLE_OVERSCAN, Math.floor(visible / 2));
    // Redraw when the visible rows leave the drawn window, the window is
    // larger than a fresh one (the view shrank) or the dates change, not on
    // every one-row scroll: the overscan is the slack between redraws.
    const inside = s.rows[0] >= drawn.first && s.rows[1] <= drawn.last;
    // One row of slack: at a fractional scroll the visible count alternates
    // between two values, which would otherwise redraw on every row.
    const fresh = drawn.last - drawn.first <= Math.min(visible + 2 * overscan + 1, 2 * visible);
    // The variance columns are held while a bar is dragged (below), so a change of that also redraws the rows.
    // The Variance button hides them (data-variance-hidden, variance-toggle.ts).
    const varianceOn = tableEl.dataset.variance === "true" && tableEl.dataset.varianceHidden === undefined;
    const holdVariance = varianceOn && s.drag?.active === true;
    const withVariance = varianceOn && !holdVariance;
    const holdResources = hasResources && s.drag?.active === true;
    const varianceMode = (holdVariance ? "hold" : withVariance ? "on" : "off") + (holdResources ? "+res" : "");
    if (inside && fresh && s.stamp === drawn.stamp && varianceMode === drawn.variance) {
      markRows(s, false);
      return;
    }
    const first = Math.max(0, s.rows[0] - overscan);
    const last = Math.min(s.rowCount, s.rows[1] + overscan);
    drawn = { first, last, stamp: s.stamp, variance: varianceMode };
    const data = JSON.parse(app.table_rows(first, last)) as (TableRow | HeaderRow)[];
    // A clicked chevron is replaced with its row; focus must not fall to the page, or the chart's keys stop working.
    const focusWasInRows = tableRows.contains(document.activeElement);
    changed.before();
    tableRows.replaceChildren(
      ...data.map((row, i) => buildRow(row, first + i, { app, rowHeight: s.rowHeight, hasCodes, hasWbs, hasPreds, hasResources, holdResources, hasProgress, holdVariance, withVariance, pending })),
    );
    changed.after();
    if (focusWasInRows) root.focus({ preventScroll: true });
    markRows(s, true);
  };
  return {
    sync: syncTable,
    invalidate: () => {
      drawn = { first: -1, last: -1, stamp: -1, variance: "off" };
    },
    showProgress: (on) => {
      showProgressColumns(on);
      fitTask();
      syncScrollSlack();
      drawn = { first: -1, last: -1, stamp: -1, variance: "off" };
    },
    setPending: (slots) => {
      pending = new Set(slots);
      pendingEpoch = shell.status().epoch;
      drawn = { first: -1, last: -1, stamp: -1, variance: "off" };
    },
    wrote: (slots) => {
      const written = new Set(slots);
      pending = new Set([...pending].filter((slot) => !written.has(slot)));
      changed.arm();
      drawn = { first: -1, last: -1, stamp: -1, variance: "off" };
    },
    reset: () => {
      drawn = { first: -1, last: -1, stamp: -1, variance: "off" };
      spacerRows = -1;
      sizeCodeTracks();
      taskNeed = measureTaskNeed(tableEl, app.longest_name() as string[]);
      showWbsColumn();
      showPredColumn();
      showResourcesColumn();
      showProgressColumns(progressOn());
      fitTask();
      scrollSlack = -1; // the new canvas has none
      syncScrollSlack();
      selectedTask = null;
      extentRow = null;
      canvasScrollY = 0;
      tableBody.scrollTop = 0;
      tableBody.scrollLeft = 0;
      tableHoverOffset = null;
      tableHoverRow = undefined;
    },
  };
}
