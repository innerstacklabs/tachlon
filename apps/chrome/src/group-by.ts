import type { ViewState } from "gantt-core";
import { hide } from "./code-filter.js";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/**
 * The Group choice in the Filters panel (gantt#1875): one entry per activity code type the open file carries, and the empty entry
 * that ungroups. It shows only when the file has activity codes (`filterInputs().activityCodes`, read at the start and on each
 * `open`). The chart holds the grouping; the choice reads it back after a change and follows the view state.
 */
export function installGroupBy({ gantt, root }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void; reset: () => void } {
  const select = menu.part<HTMLSelectElement>("group-by");
  // Typing a letter in a choice jumps to an option; the chart's keys must not also act on it.
  select.addEventListener("keydown", (e) => e.stopPropagation());
  const show = (state: ViewState): void => {
    const wanted = state.groupBy ?? "";
    if (select.value !== wanted) select.value = wanted;
  };
  select.addEventListener("change", () => {
    gantt.commands.setGroupBy(select.value === "" ? null : select.value);
    // A refused change (a bar press) puts the choice back on the grouping the chart holds.
    show(gantt.state());
    refocus(root);
  });
  const reset = (): void => {
    const types = gantt.filterInputs().activityCodes.map(([name]) => name);
    select.replaceChildren(new Option("Group", ""), ...types.map((name) => new Option(name, name)));
    hide(select, types.length === 0);
    show(gantt.state());
  };
  reset();
  return { sync: show, reset };
}
