import type { ViewState } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

/** Write a `hidden` only when it changes: a repeated write is still a DOM mutation. */
export const hide = (element: HTMLElement, hidden: boolean): void => {
  if (element.hidden !== hidden) element.hidden = hidden;
};

const option = (value: string, text: string): HTMLOptionElement => {
  const o = document.createElement("option");
  o.value = value;
  o.textContent = text;
  return o;
};

/**
 * The activity code filter (gs#703): a code type, then one of its values, then "Clear". The group shows only when the open file has
 * activity codes (`filterInputs().activityCodes`, read at the start and on each `open`), and the value choice only once a type is
 * chosen. The chart holds the filter; this shows it from the view state, and writes the DOM only when the filter it shows changes.
 */
export function installCodeFilter({ gantt, root }: ChromeContext, menu: Piece): { sync: (state: ViewState) => void; reset: () => void } {
  const group = menu.part<HTMLElement>("code-filter");
  const codeType = menu.part<HTMLSelectElement>("code-type");
  const codeValue = menu.part<HTMLSelectElement>("code-value");
  const clear = menu.part<HTMLButtonElement>("code-clear");
  let choices: [string, string[]][] = [];
  /** The filter shown (`Phase = Civil`), or an empty string while it is off. */
  let shown = "";
  /** The type and value of the filter that is on, or two empty strings: what both lists show whenever a change is refused. */
  let on: [string, string] = ["", ""];
  const fillValues = (type: string): void => {
    const values = choices.find(([name]) => name === type)?.[1] ?? [];
    codeValue.replaceChildren(option("", "Value"), ...values.map((value) => option(value, value)));
    hide(codeValue, type === "");
  };
  /** Show the filter the chart holds: the Clear button and its pressed state, and the choices back at rest once it is off. */
  const show = (state: ViewState): void => {
    const code = state.codeFilter;
    const label = code === null ? "" : `${code.type} = ${code.value}`;
    if (label === shown) return;
    shown = label;
    clear.setAttribute("aria-pressed", String(code !== null));
    hide(clear, code === null);
    if (code === null) {
      on = ["", ""];
      codeType.value = "";
      fillValues("");
    }
  };
  const reset = (): void => {
    choices = gantt.filterInputs().activityCodes;
    shown = "";
    on = ["", ""];
    hide(group, choices.length === 0);
    codeType.replaceChildren(option("", "Activity code"), ...choices.map(([name]) => option(name, name)));
    hide(codeValue, true);
    hide(clear, true);
    clear.setAttribute("aria-pressed", "false");
    show(gantt.state());
  };
  const offIfCode = (): void => {
    if (gantt.state().filter === "code") gantt.commands.setFilter("off");
  };
  const edit = (run: () => void): void => {
    if (!gantt.busy()) run();
    refocus(root);
  };
  for (const select of [codeType, codeValue]) {
    // Typing a letter in a choice jumps to an option; the chart's keys must not also act on it.
    select.addEventListener("keydown", (e) => e.stopPropagation());
  }
  codeType.addEventListener("change", () => {
    const type = codeType.value;
    if (gantt.busy()) {
      // Refused like the other filters: both lists go back to the filter that is on.
      codeType.value = on[0];
      fillValues(on[0]);
      codeValue.value = on[1];
      refocus(root);
      return;
    }
    offIfCode();
    show(gantt.state());
    codeType.value = type;
    fillValues(type);
  });
  codeValue.addEventListener("change", () => {
    const [type, value] = [codeType.value, codeValue.value];
    if (value === "") return;
    edit(() => {
      gantt.commands.setFilter({ code: { type, value } });
      const state = gantt.state();
      show(state);
      if (state.codeFilter !== null) on = [type, value];
    });
    // Outside the guarded callback, so a refused change resets the list too.
    codeValue.value = type === on[0] ? on[1] : "";
  });
  clear.addEventListener("click", () =>
    edit(() => {
      offIfCode();
      show(gantt.state());
    }),
  );
  reset();
  return { sync: show, reset };
}
