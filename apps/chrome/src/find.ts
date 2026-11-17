import type { ViewState } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

export interface Find {
  /** Put focus in the find box with its text selected (the chart's find key, through `on("find")`). */
  focus: () => void;
  /** Follow the filters through `change`: search again from the first match when a filter or the lookahead window changed under the query. The first call only records them. */
  follow: (state: ViewState) => void;
  /** Count again: the schedule changed (an `edit`, a `move`). */
  recount: () => void;
  /** A new schedule was opened: forget the query and the matches. */
  reset: () => void;
}

/** The filters a search covers, as one string: a change of it is a change of the rows find searches. */
const filtersOf = (v: ViewState): string =>
  JSON.stringify([v.filter, v.statusFilter, v.checkFilter, v.codeFilter, v.resourceFilter, v.lookahead.on && v.lookahead]);

/** Whether `text` is a whole, real `YYYY-MM-DD` date (2026-02-30 is not). `commands.scrollTo` has the last word on Enter. */
function isDate(text: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (m === null) return false;
  const [year, month, day] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(0);
  date.setUTCFullYear(year, month - 1, day);
  return date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
}

/** The find box: it searches through `commands.find`, goes to a match with `commands.findGo` and to a date with `commands.scrollTo`. */
export function installFind({ gantt, root }: ChromeContext, bar: Piece): Find {
  const { commands } = gantt;
  const input = bar.part<HTMLInputElement>("find-input");
  const countText = bar.part<HTMLSpanElement>("find-count");
  const fmt = new Intl.NumberFormat("en-US");
  const active = (): boolean => input.value !== "";
  let count = 0;
  let index = 0;
  let filters: string | null = null;
  // The row and task counts the last `change` carried: a collapse or an expand changes the rows the search covers.
  let rows = "";
  // A recount is waiting for the frames of an edit to end.
  let queued = false;
  // A query that is a whole real date waits for Enter to go to its date: the keystroke that completes it does not jump to a name that holds it.
  let heldForDate = false;
  // With a row filter or Lookahead on, the search covers only the shown rows, so an empty result says so (gs#703). The sentence
  // is the note's title, not its text: the visible suffix did not fit the toolbar with the import notice and a crowded filter at 1280 px.
  const searchedShownOnly = (): boolean => {
    const state = gantt.state();
    return state.filter !== "off" || state.lookahead.on;
  };
  const showCount = (): void => {
    countText.textContent = input.value === "" ? "" : count === 0 ? "no matches" : `${fmt.format(index + 1)} of ${fmt.format(count)}`;
    if (input.value !== "" && count === 0 && searchedShownOnly()) countText.title = "no matches in the shown rows";
    else countText.removeAttribute("title");
  };
  const refresh = (): void => {
    const found = commands.find(input.value);
    count = typeof found === "number" ? found : 0;
    index = Math.min(index, Math.max(count - 1, 0));
    showCount();
  };
  const searchFromFirst = (): void => {
    index = 0;
    refresh();
    if (count > 0) commands.findGo(0);
  };
  const rerun = (): void => {
    heldForDate = isDate(input.value);
    if (heldForDate) {
      index = 0;
      refresh();
    } else {
      searchFromFirst();
    }
  };
  /** Count again once the schedule changed (an `edit`, a `move`, a collapse). A drag changes the schedule each frame and is not counted;
   * its release is, once, two frames on: the release fires `move`, and the frame after it can fire the edit that records it. */
  const recount = (): void => {
    if (queued || !active() || gantt.busy()) return;
    queued = true;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        queued = false;
        if (active() && !gantt.busy()) refresh();
      }),
    );
  };
  input.addEventListener("input", rerun);
  input.addEventListener("keydown", (e) => {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      if (heldForDate && input.value !== "") {
        if (commands.scrollTo({ date: input.value }) === true) {
          countText.textContent = "went to date";
          return;
        }
        // The chart refused the date after all: search the names from the first match, as typing would have.
        heldForDate = false;
        searchFromFirst();
      } else {
        // The chart keeps one match list: search the box's own query again first, so Enter goes to the box's matches.
        refresh();
        if (count === 0) return showCount();
        const next = e.shiftKey ? (index - 1 + count) % count : (index + 1) % count;
        if (commands.findGo(next) === true) index = next;
      }
      showCount();
    } else if (e.key === "Escape") {
      // Clear the search and give focus back to the chart, so its keys work again.
      e.preventDefault();
      e.stopPropagation();
      input.value = "";
      commands.find("");
      count = 0;
      index = 0;
      showCount();
      refocus(root);
    } else if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      input.select();
    }
  });
  return {
    focus: () => {
      input.focus();
      input.select();
    },
    follow: (state) => {
      const now = filtersOf(state);
      const counts = `${state.rowCount} ${state.taskCount}`;
      if (filters !== null && now !== filters && active()) rerun();
      else if (counts !== rows) recount();
      filters = now;
      rows = counts;
    },
    recount,
    reset: () => {
      input.value = "";
      count = 0;
      index = 0;
      showCount();
    },
  };
}
