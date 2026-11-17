import { type HeaderRow, type PublicApp, type Status, type TableRow } from "./api.js";

export interface FilterScroll {
  /** Called with each frame's status: the filter it reads is compared with the last one; see `installFilterScroll`. */
  sync: (s: Status) => void;
}

/**
 * Turning a filter on scrolls the chart to the rows it shows (gs#1015). A person who turns on a filter and sees no bars cannot tell
 * why; the rows are there, in another part of the timeline. When the filter (or the lookahead window) changes to one that is on, the
 * next frame after the change is read once: if rows are left and no bar was drawn, `App.go_to_date` pans to the first row's start
 * day (keeping the zoom, and scrolling that row to the top), which the chart animates as it does for the Find box's date jump. A
 * filter whose bars are already in view leaves the chart where it is, and so does a filter turned off.
 *
 * It is one scroll per change, not per-frame work. Each frame `sync` compares the status's filter name and lookahead window with the
 * last ones (no allocation, no call into the app) and, in the single frame after a change, reads `Status.bars` and the first row. The
 * activity code and Status choices keep the filter name while their value changes, so a change of value alone does not scroll.
 */
export function installFilterScroll(app: PublicApp): FilterScroll {
  let filter: Status["filter"] | null = null;
  let lookOn = false;
  let lookStart = "";
  let lookWeeks = 0;
  /** The frame count at the change, or -1 when no change is waiting for its frame. */
  let armed = -1;
  /** Pans to the first task row's start; a group header has no task and no start, so it is skipped. */
  const scroll = (rows: number): void => {
    for (let at = 0; at < rows; at += 64) {
      const task = (JSON.parse(app.table_rows(at, at + 64)) as (TableRow | HeaderRow)[]).find((row): row is TableRow => row[0] !== null);
      if (task !== undefined) return void app.go_to_date(task[1].slice(0, 10));
    }
  };
  return {
    sync: (s) => {
      const look = s.lookahead;
      if (s.filter !== filter || look.on !== lookOn || (look.on && (look.start !== lookStart || look.weeks !== lookWeeks))) {
        // The first status only sets the baseline: a schedule that opens with a filter on is not a change.
        armed = filter !== null && (s.filter !== "off" || look.on) ? s.frames : -1;
        filter = s.filter;
        lookOn = look.on;
        lookStart = look.start;
        lookWeeks = look.weeks;
        return;
      }
      if (armed < 0 || s.frames <= armed) return;
      armed = -1;
      if (s.rowCount > 0 && s.bars === 0) scroll(s.rowCount);
    },
  };
}
