import { type Status, type TaskDetails } from "./api.js";
import { barSentence } from "./bar-speech.js";
import { type CellEdit } from "./cell-edit.js";
import { element } from "./dom.js";
import { nameOf } from "./field-names.js";
import { type Shell } from "./shell.js";

export interface HoverCard {
  /** Show, fill and place the card for the hovered task. */
  sync: (s: Status) => void;
  /** A new schedule was opened: hide the card. */
  reset: () => void;
  /** Show the card of `task` under its row, ahead of the hover, until `unpin`, a pointer move over the canvas or a new schedule. */
  pin: (task: number) => void;
  /** Drop the pinned card; the hover card works as before. */
  unpin: () => void;
  /** Whether a card is pinned. */
  pinned: () => boolean;
}

/** The hover card. `pointerAt` gives the last pointer position over the canvas, in client pixels. */
export function installCard( // oxlint-disable-line max-lines-per-function -- existing, over cap
  { app, root }: Shell,
  edit: CellEdit,
  pointerAt: () => [number, number] | null,
): HoverCard {
  const card = element<HTMLDivElement>(root, "hover-card");
  // Hidden status region: Alt+Enter says what the card shows, unpin and reset empty it. Hover stays silent.
  const live = element<HTMLDivElement>(root, "card-live");
  let shownCardKey: string | null = null;
  let cardSize: [number, number] = [0, 0];
  let positionedAt: [number, number] | null = null;
  // The pinned task (Alt+Enter) and where the pointer was when it was pinned; a different position unpins.
  let pinnedTask: number | null = null;
  let pinnedPointer: [number, number] | null = null;
  // Where the previous pinned card's row was, to tell which way the cursor is travelling.
  let pinnedRowTop: number | null = null;
  const samePoint = (a: [number, number] | null, b: [number, number] | null): boolean =>
    a === b || (a !== null && b !== null && a[0] === b[0] && a[1] === b[1]);
  const n = (key: string): string => nameOf(root, key);
  const cardRow = (grid: HTMLDivElement, label: string, value: string, critical = false): void => {
    const labelEl = document.createElement("div");
    labelEl.className = "label";
    labelEl.textContent = label;
    const valueEl = document.createElement("div");
    valueEl.className = `value${critical ? " critical" : ""}`;
    valueEl.textContent = value;
    grid.append(labelEl, valueEl);
  };
  const relatedTasks = (names: string[], count: number): string => {
    const lines = [...names];
    if (count > names.length) lines.push(`+ ${count - names.length} more`);
    return lines.length > 0 ? lines.join("\n") : "—";
  };
  const fillCard = (details: TaskDetails): void => {
    const title = document.createElement("div");
    title.className = "title";
    title.textContent = details.activityId === "" ? details.name : `${details.activityId} ${details.name}`;
    const path = document.createElement("div");
    path.className = "outline-path";
    path.textContent = details.outlinePath;
    const grid = document.createElement("div");
    grid.className = "details";
    cardRow(grid, n("start"), details.start);
    cardRow(grid, n("finish"), details.finish);
    if (!details.summary) {
      const constraint = app.task_constraint_text(details.task);
      if (constraint !== "") cardRow(grid, n("start-no-earlier"), constraint);
      const deadline = app.task_deadline_text(details.task);
      if (deadline !== "") cardRow(grid, n("deadline"), deadline);
      cardRow(grid, n("duration"), app.task_duration(details.task));
      if (details.actualStart !== "") cardRow(grid, n("remaining"), app.task_remaining(details.task));
      cardRow(grid, n("late-start"), details.lateStart);
      cardRow(grid, n("late-finish"), details.lateFinish);
      const floatCalendar = app.task_float_calendar_text(details.task);
      cardRow(grid, n("float"), floatCalendar === "" ? details.floatText : `${details.floatText} (${floatCalendar})`);
      cardRow(grid, n("calendar"), app.task_calendar_text(details.task));
      const { resources } = details;
      if (resources.length > 0) cardRow(grid, n("resources"), [...resources.slice(0, 3), ...(resources.length > 3 ? [`+${resources.length - 3}`] : [])].join(", "));
      cardRow(grid, n("critical"), details.critical ? "yes" : "no", details.critical);
      const drivers = app.task_driven_by(details.task);
      if (drivers !== "") cardRow(grid, n("driven-by"), drivers);
      cardRow(grid, n("status"), details.status);
      if (details.actualStart !== "") cardRow(grid, n("actual-start"), details.actualStart);
      if (details.actualFinish !== "") cardRow(grid, n("actual-finish"), details.actualFinish);
      if (details.percentComplete !== "") cardRow(grid, n("percent-head"), details.percentComplete);
      const [startMoved, finishMoved] = details.baselineVariance ?? [null, null];
      if (details.baselineStart !== undefined) cardRow(grid, n("baseline-start"), startMoved === null ? details.baselineStart : `${details.baselineStart} (${startMoved})`);
      if (details.baselineFinish !== undefined) cardRow(grid, n("baseline-finish"), finishMoved === null ? details.baselineFinish : `${details.baselineFinish} (${finishMoved})`);
    }
    const wbs = app.task_wbs_code(details.task);
    if (wbs !== "") cardRow(grid, n("wbs"), wbs);
    for (const [codeType, value] of JSON.parse(app.task_activity_codes(details.task)) as [string, string][]) cardRow(grid, codeType, value);
    cardRow(grid, `${n("predecessors")} (${details.predecessorCount})`, relatedTasks(details.predecessors, details.predecessorCount));
    cardRow(grid, `${n("successors")} (${details.successorCount})`, relatedTasks(details.successors, details.successorCount));
    card.replaceChildren(title, path, grid);
  };
  return {
    sync: (s) => {
      const pointer = pointerAt();
      // A cell edit drops the pin too: its card must not come back when the edit closes.
      if (pinnedTask !== null && (edit.active() || !samePoint(pointer, pinnedPointer))) {
        pinnedTask = null;
        live.textContent = "";
      }
      const shown = !edit.active() && s.gesture === "idle";
      // The pinned card needs its row on screen; with the row not drawn there is nowhere to put it, so no card shows.
      const rowEl = pinnedTask === null ? null : root.querySelector<HTMLElement>(`[data-part=table-rows] .row[data-task="${pinnedTask}"]`);
      const task = !shown ? null : pinnedTask === null ? s.hover?.task ?? null : rowEl === null ? null : pinnedTask;
      const cardKey = task === null ? null : `${task}:${s.stamp}:${pinnedTask === null ? "" : "pinned"}`;
      const cardChanged = cardKey !== shownCardKey;
      if (cardChanged) {
        shownCardKey = cardKey;
        if (task === null) {
          card.hidden = true;
        } else {
          const details = app.task_details(task) as TaskDetails;
          fillCard(details);
          card.dataset.task = String(task);
          card.hidden = false;
          cardSize = [card.offsetWidth, card.offsetHeight];
        }
      }
      if (!card.hidden && pinnedTask !== null) {
        if (cardChanged) {
          // Over the chart pane beside the selected row, so the Task column of the rows around it stays readable. When the pane is
          // too narrow, on the side of the row away from the cursor's travel (down unless the selection last moved up).
          const bounds = root.getBoundingClientRect();
          const rowBox = rowEl?.getBoundingClientRect();
          const nameBox = rowEl?.querySelector<HTMLElement>(":scope > .name")?.getBoundingClientRect() ?? rowBox;
          const [cardWidth, cardHeight] = cardSize;
          let x = 0;
          let y = 0;
          if (rowBox !== undefined && nameBox !== undefined) {
            const rowTop = rowBox.top - bounds.top;
            const beside = nameBox.right - bounds.left + 14;
            const movingUp = pinnedRowTop !== null && rowTop < pinnedRowTop;
            pinnedRowTop = rowTop;
            if (beside + cardWidth <= root.clientWidth) {
              x = beside;
              y = Math.min(rowTop, root.clientHeight - cardHeight);
            } else {
              x = Math.min(rowBox.left - bounds.left + 14, root.clientWidth - cardWidth);
              const below = rowBox.bottom - bounds.top;
              const above = rowTop - cardHeight;
              y = movingUp ? (below + cardHeight <= root.clientHeight ? below : above) : above >= 0 ? above : below;
            }
          }
          card.style.transform = `translate(${Math.max(0, x)}px, ${Math.max(0, y)}px)`;
          positionedAt = null;
        }
      } else if (!card.hidden && pointer !== null && (cardChanged || positionedAt?.[0] !== pointer[0] || positionedAt[1] !== pointer[1])) {
        // The card is inside the root and positioned against it, so the pointer and the clamp are in the root's own box.
        const bounds = root.getBoundingClientRect();
        const pointerX = pointer[0] - bounds.left;
        const pointerY = pointer[1] - bounds.top;
        const [cardWidth, cardHeight] = cardSize;
        const x = pointerX + 14 + cardWidth <= root.clientWidth ? pointerX + 14 : pointerX - 14 - cardWidth;
        const y = pointerY + 14 + cardHeight <= root.clientHeight ? pointerY + 14 : pointerY - 14 - cardHeight;
        card.style.transform = `translate(${Math.max(0, x)}px, ${Math.max(0, y)}px)`;
        positionedAt = pointer;
      }
    },
    reset: () => {
      pinnedTask = null;
      live.textContent = "";
      shownCardKey = null;
      card.hidden = true;
    },
    pin: (task) => {
      pinnedTask = task;
      pinnedPointer = pointerAt();
      const details = app.task_details(task) as TaskDetails;
      live.textContent = barSentence(root, details);
    },
    unpin: () => {
      pinnedTask = null;
      live.textContent = "";
    },
    pinned: () => pinnedTask !== null,
  };
}
