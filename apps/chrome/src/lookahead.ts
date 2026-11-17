import type { ViewState } from "gantt-core";
import { hide } from "./code-filter.js";
import type { ChromeContext, Piece } from "./parts.js";

/**
 * The lookahead window (gs#96): a start date and a length in weeks. The inputs hold what the reader typed and the chart holds the
 * window; a value the chart refuses leaves the old window and marks the inputs invalid. The inputs show the chart's window at the start
 * and after each `open`.
 */
export function installLookahead({ gantt }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void; reset: () => void } {
  const toggle = menu.part<HTMLButtonElement>("lookahead-toggle");
  const start = menu.part<HTMLInputElement>("lookahead-start");
  const weeks = menu.part<HTMLInputElement>("lookahead-weeks");
  const window = menu.part<HTMLSpanElement>("lookahead-window");
  const reset = (): void => {
    const w = gantt.state().lookahead;
    start.value = w.start;
    weeks.value = String(w.weeks);
    start.removeAttribute("aria-invalid");
    weeks.removeAttribute("aria-invalid");
  };
  const apply = (): void => {
    const n = Number(weeks.value);
    const set = Number.isInteger(n) && n >= 1 && gantt.commands.setFilter({ lookahead: { start: start.value, weeks: n } }) === true;
    start.setAttribute("aria-invalid", String(!set));
    weeks.setAttribute("aria-invalid", String(!set));
  };
  toggle.addEventListener("click", () => {
    if (!gantt.state().lookahead.on) apply();
    else gantt.commands.setFilter({ lookahead: null });
  });
  for (const input of [start, weeks]) {
    input.addEventListener("change", () => {
      if (gantt.state().lookahead.on) apply();
    });
  }
  reset();
  return {
    sync: (state) => {
      toggle.setAttribute("aria-pressed", String(state.lookahead.on));
      hide(window, !state.lookahead.on);
    },
    reset,
  };
}
