import { optional } from "./dom.js";
import { type Shell, listening, watching } from "./shell.js";

const SET_VARIANCE = "gantt:set-variance";

/**
 * Show or hide the Start, Finish and Float variance columns, with the toolbar's button or without it: the button's click and `applyColumns` (a layout put back) both end here, so the button's
 * `aria-pressed` and `data-variance-hidden` stay in step when the button exists, and the flag is set when it does not.
 */
export function setVariance(root: EventTarget, on: boolean): void {
  root.dispatchEvent(new CustomEvent<boolean>(SET_VARIANCE, { detail: on }));
}

/**
 * The Variance button (gs#703): while a baseline is set it hides and shows the Start, Finish and Float variance columns, which are
 * on by default. It sets `data-variance-hidden` on the table; the table and its stylesheet leave the columns out while it is set.
 * Nothing here runs per frame. The state changes on a click and when the baseline changes: the toolbar writes the chart root's
 * `data-baseline` (the baseline's one-line text) when the number of baselined rows changes, and that is what this watches. With
 * no baseline the button is hidden and the columns are back on for the next one; a new baseline text (a new baseline, a file
 * opened with one) also brings them back. An edit changes neither, so the columns stay as the scheduler left them.
 */
export function installVarianceToggle(shell: Shell): void {
  const { root } = shell;
  // The chart works with the toolbar removed: without the button there is nothing to click and nothing to watch, but `setVariance` still sets the flag.
  const button = optional<HTMLButtonElement>(root, "variance-toggle");
  const table = root.querySelector<HTMLElement>("[data-part=table]");
  if (table === null) return;
  const show = (on: boolean): void => {
    if (on) delete table.dataset.varianceHidden;
    else table.dataset.varianceHidden = "true";
    button?.setAttribute("aria-pressed", String(on));
  };
  listening(shell, root, SET_VARIANCE, ((event: CustomEvent<boolean>) => show(event.detail)) as (event: never) => void);
  if (button === null) return;
  const baseline = (): void => {
    button.hidden = root.dataset.baseline === undefined;
    show(true);
  };
  button.addEventListener("click", () => {
    setVariance(root, table.dataset.varianceHidden !== undefined);
    root.focus({ preventScroll: true });
  });
  watching(shell, new MutationObserver(baseline)).observe(root, { attributes: true, attributeFilter: ["data-baseline"] });
  baseline();
}
