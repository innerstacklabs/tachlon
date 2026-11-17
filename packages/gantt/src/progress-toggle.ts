import { optional } from "./dom.js";
import { type Shell, listening } from "./shell.js";

const SET_PROGRESS = "gantt:set-progress";

/**
 * Show or hide the four progress columns, with the toolbar's button or without it: the button's click and `applyColumns` (a layout put back) both end here, so the button's `aria-pressed` and the table's flag
 * stay in step when the button exists, and the flag is set when it does not.
 */
export function setProgress(root: EventTarget, on: boolean): void {
  root.dispatchEvent(new CustomEvent<boolean>(SET_PROGRESS, { detail: on }));
}

export interface ProgressToggle {
  /** Whether the four progress columns are shown. */
  on: () => boolean;
  /** A file was opened: the columns are on when it has actuals, off when it has none. */
  opened: () => void;
}

/**
 * The Progress button: it shows the Actual start, Actual finish, Remaining and % complete columns, empty, on a schedule
 * that has no actuals, and takes them away again. The state changes on a click and when a file opens, never per frame;
 * `show` is the table's own once-per-change redraw.
 */
export function installProgressToggle(shell: Shell, show: (on: boolean) => void): ProgressToggle {
  const { app, root } = shell;
  // The chart works with the toolbar removed: without the button there is nothing to click, and the state still follows each opened file.
  const button = optional<HTMLButtonElement>(root, "progress-toggle");
  let on = app.has_progress();
  const set = (next: boolean): void => {
    on = next;
    button?.setAttribute("aria-pressed", String(on));
    show(on);
  };
  button?.addEventListener("click", () => setProgress(root, !on));
  listening(shell, root, SET_PROGRESS, ((event: CustomEvent<boolean>) => {
    if (event.detail !== on) set(event.detail);
  }) as (event: never) => void);
  button?.setAttribute("aria-pressed", String(on));
  return {
    on: () => on,
    // The table's reset follows at the next frame and reads the state, so only the button is written here.
    opened: () => {
      on = app.has_progress();
      button?.setAttribute("aria-pressed", String(on));
    },
  };
}
