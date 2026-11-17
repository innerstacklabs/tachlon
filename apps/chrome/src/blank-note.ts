import type { ViewState } from "gantt-core";
import type { Piece } from "./parts.js";

/** The line when a filter or the Lookahead window keeps no row of a schedule that has tasks (gantt#703). */
const FILTERED_OUT = "No rows match. A filter or the Lookahead window is on; press Clear filters.";

/**
 * The line in the task list of a schedule with no rows (gantt#703). With no tasks it says how to add the first one, goes as soon as there is
 * a task and returns when the last one goes. With tasks and every row hidden by a filter or the Lookahead window it says so and where Clear
 * filters is. It is the one empty-list element: it keeps the no-tasks text it came with and swaps it for the filter text and back.
 */
export function installBlankNote(note: Piece): (state: ViewState) => void {
  const line = note.part<HTMLParagraphElement>("table-empty");
  const blank = line.textContent ?? "";
  return ({ taskCount, rowCount }) => {
    const hide = taskCount > 0 && rowCount > 0;
    const text = taskCount > 0 ? FILTERED_OUT : blank;
    if (line.hidden !== hide) line.hidden = hide;
    if (line.textContent !== text) line.textContent = text;
  };
}
