import type { ViewState } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/**
 * The Check choice in the Filters panel (gs#703): ten schedule checks and the empty entry that clears it. The
 * chart holds the filter; the choice reads it back after a change and, on each `change`, falls back to the empty entry when another
 * filter has replaced it.
 */
export function installCheckFilter({ gantt, root }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void } {
  const select = menu.part<HTMLSelectElement>("check-filter");
  // Typing a letter in a choice jumps to an option; the chart's keys must not also act on it.
  select.addEventListener("keydown", (e) => e.stopPropagation());
  select.addEventListener("change", () => {
    const value = select.value;
    const kinds = ["missing-logic", "open-start", "open-end", "high-float", "high-duration", "invalid-dates", "missing-resources", "missed", "hard-constraints", "negative-float"] as const;
    if (kinds.includes(value as (typeof kinds)[number])) gantt.commands.setFilter({ check: value as (typeof kinds)[number] });
    else if (gantt.state().filter === "check") gantt.commands.setFilter("off");
    // A refused change (a bar press) puts the choice back on the filter the chart holds.
    select.value = gantt.state().checkFilter ?? "";
    refocus(root);
  });
  return {
    sync: (state) => {
      if (state.filter !== "check" && select.value !== "") select.value = "";
    },
  };
}
