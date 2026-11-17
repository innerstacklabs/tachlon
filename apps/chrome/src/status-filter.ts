import type { ViewState } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/**
 * The Status choice in the Filters panel (gs#703): In progress, Not started or Complete, and the empty entry that clears it. The chart
 * holds the filter; the choice reads it back after a change and, on each `change`, falls back to the empty entry when another
 * filter has replaced it.
 */
export function installStatusFilter({ gantt, root }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void } {
  const select = menu.part<HTMLSelectElement>("status-filter");
  // Typing a letter in a choice jumps to an option; the chart's keys must not also act on it.
  select.addEventListener("keydown", (e) => e.stopPropagation());
  select.addEventListener("change", () => {
    const value = select.value;
    if (value === "in-progress" || value === "not-started" || value === "complete") gantt.commands.setFilter({ status: value });
    else if (gantt.state().filter === "status") gantt.commands.setFilter("off");
    // A refused change (a bar press) puts the choice back on the filter the chart holds.
    select.value = gantt.state().statusFilter ?? "";
    refocus(root);
  });
  return {
    sync: (state) => {
      if (state.filter !== "status" && select.value !== "") select.value = "";
    },
  };
}
