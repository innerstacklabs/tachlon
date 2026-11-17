import type { EditResult, ViewState } from "gantt-core";
import { hide } from "./code-filter.js";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

const fmt = new Intl.NumberFormat("en-US");

/** The baseline's one-line confirmation: `Baseline set at data date 14 Oct: 59 rows`, or `Baseline set: 59 rows` when it was set with no data date. */
const baselineText = (rows: number, day: string): string => `Baseline set${day === "" ? "" : ` at data date ${day}`}: ${fmt.format(rows)} ${rows === 1 ? "row" : "rows"}`;

/**
 * The baseline (gs#703): "Baseline" keeps the dates as they are, "Add new" gives the tasks added since the baseline their current dates and
 * keeps every other entry, "Clear" drops it. The buttons press `commands.setBaseline`, `addNewToBaseline` and `clearBaseline` and show what
 * `state().baseline` says: the DOM is written only when the number of baselined rows, the day it was set at (a second set over the same rows
 * says a new day) or the added tasks change. Returns the call that shows a state.
 */
export function installBaselineButtons({ gantt, root }: ChromeContext, bar: Piece): (state: ViewState) => void {
  const set = bar.part<HTMLButtonElement>("baseline-set");
  const add = bar.part<HTMLButtonElement>("baseline-add");
  const clear = bar.part<HTMLButtonElement>("baseline-clear");
  const idle = set.title;
  let rows = 0;
  let day = "";
  let added = 0;
  const show = ({ baseline }: ViewState): void => {
    const nextRows = baseline?.rows ?? 0;
    const nextDay = baseline?.dayText ?? "";
    const nextAdded = baseline?.added ?? 0;
    if (nextRows === rows && nextDay === day && nextAdded === added) return;
    if (nextRows !== rows || nextDay !== day) {
      set.setAttribute("aria-pressed", String(nextRows > 0));
      set.title = nextRows > 0 ? `${baselineText(nextRows, nextDay)}. Click to keep the dates as they are now instead.` : idle;
      hide(clear, nextRows === 0);
    }
    if (nextAdded !== added) hide(add, nextAdded === 0);
    [rows, day, added] = [nextRows, nextDay, nextAdded];
  };
  const press = (button: HTMLButtonElement, run: () => EditResult | undefined): void => {
    button.addEventListener("click", () => {
      const answer = run();
      if (answer?.ok === false && answer.reason !== "busy") console.warn(`gantt: ${answer.message}`);
      show(gantt.state());
      refocus(root);
    });
  };
  press(set, () => gantt.commands.setBaseline());
  press(add, () => gantt.commands.addNewToBaseline());
  press(clear, () => gantt.commands.clearBaseline());
  return show;
}
