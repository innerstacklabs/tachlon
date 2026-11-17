import type { ViewState } from "gantt-core";
import { hide } from "./code-filter.js";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/**
 * The Resource choice in the Filters panel (gs#703): one entry per resource name the open file gives its tasks, and the empty entry
 * that clears it. It shows only when the file has resources (`filterInputs().resources`, read at the start and on each `open`). The
 * chart holds the filter; the choice reads it back after a change and, on each `change`, follows the view state, so another filter
 * (or a host's `setFilter`) moves it.
 */
export function installResourceFilter({ gantt, root }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void; reset: () => void } {
  const select = menu.part<HTMLSelectElement>("resource-filter");
  // Typing a letter in a choice jumps to an option; the chart's keys must not also act on it.
  select.addEventListener("keydown", (e) => e.stopPropagation());
  const show = (state: ViewState): void => {
    const wanted = state.resourceFilter ?? "";
    if (select.value !== wanted) select.value = wanted;
  };
  select.addEventListener("change", () => {
    const name = select.value;
    if (name !== "") gantt.commands.setFilter({ resource: name });
    else if (gantt.state().filter === "resource") gantt.commands.setFilter("off");
    // A refused change (a bar press) puts the choice back on the filter the chart holds.
    show(gantt.state());
    refocus(root);
  });
  const reset = (): void => {
    const names = gantt.filterInputs().resources;
    select.replaceChildren(new Option("Resource", ""), ...names.map((name) => new Option(name, name)));
    hide(select, names.length === 0);
    show(gantt.state());
  };
  reset();
  return { sync: show, reset };
}
