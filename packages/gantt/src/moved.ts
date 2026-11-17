import { type Status } from "./api.js";
import { installBaselineSummary } from "./baseline-summary.js";
import { installDragNote } from "./drag-note.js";
import { type Shell } from "./shell.js";

export interface MovedNote {
  /** Show, fill and fade the note for the status's move report; write only what changed. */
  sync: (s: Status) => void;
  /** A new schedule was opened: hide the note. */
  reset: () => void;
}

/**
 * The "what moved" note: one line at the top right of the canvas, `8 tasks moved · finish +7 days`, shown at full opacity
 * after an edit or a drag release that moved other tasks, then faded out by the app's own `status.moved.alpha`. The page
 * keeps what it last wrote, so an idle frame (the same report, the same two-decimal opacity) writes nothing.
 *
 * The baseline summary chip (`baseline-summary.ts`) is written by the same call.
 *
 * The Show moved button is a sibling of the note, never inside the live region. It is shown while a report exists and the
 * rows are not already the moved ones, and written only when that changes; clicking it turns the Moved filter on.
 *
 * The note is a live region and stays in the accessibility tree: hiding it is opacity 0 and no text, never `hidden` or
 * `display: none`, which some screen readers answer by missing the text set in the tick that unhides it. A report that
 * arrives while the note is hidden first makes it visible, and its text is written on the next frame.
 *
 * While the pointer is over the note the app keeps the report fully visible (`App.set_moved_hold`): `pointerenter` and
 * `pointerleave` on the note tell it, and hiding or resetting the note ends the hold, so the hold never outlives the text
 * under the pointer. The note takes the pointer only while it has text (`moved.css`).
 */
export function installMoved(shell: Shell): MovedNote { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { root, app } = shell;
  // The baseline summary is the moved note's standing companion: the note says what the last move did, the chip says where the schedule stands.
  const summary = installBaselineSummary(shell);
  // The drag note (a finished task that did not move) is written by the same call.
  const dragNote = installDragNote(shell);
  const note = root.querySelector<HTMLDivElement>("[data-part=moved-note]");
  if (note === null) throw new Error("missing #moved-note");
  const button = root.querySelector<HTMLButtonElement>("[data-part=moved-show]");
  if (button === null) throw new Error("missing #moved-show");
  /** Whether the button is shown: a report exists and the rows are not already the moved ones. */
  let buttonShown = false;
  const showButton = (on: boolean): void => {
    if (on !== buttonShown) {
      buttonShown = on;
      button.hidden = !on;
    }
  };
  button.addEventListener("click", () => {
    app.set_moved_only(true);
  });
  /** The `seq` whose text is in the note, or null when it holds no text. */
  let textSeq: number | null = null;
  let visible = false;
  /** The hold the page last told the app about. */
  let held = false;
  /** The last opacity written, to two decimals. */
  let shownOpacity: string | null = null;
  const setOpacity = (opacity: string): void => {
    if (opacity !== shownOpacity) {
      shownOpacity = opacity;
      note.style.opacity = opacity;
    }
  };
  const setHold = (on: boolean): void => {
    if (on !== held) {
      held = on;
      app.set_moved_hold(on);
    }
  };
  note.addEventListener("pointerenter", () => {
    if (visible) setHold(true);
  });
  note.addEventListener("pointerleave", () => setHold(false));
  const hide = (): void => {
    setHold(false);
    textSeq = null;
    if (visible) {
      visible = false;
      setOpacity("0");
      note.textContent = "";
    }
  };
  return {
    sync: (s) => {
      summary.sync(s);
      dragNote.sync(s);
      const moved = s.moved;
      // The report stays in the status after its note fades, and so does the button, until the next change.
      showButton(moved !== null && s.filter !== "moved");
      // The report stays in the status after its alpha reaches 0, so a faded one is hidden, never shown again.
      if (moved === null || moved.alpha <= 0) {
        hide();
        return;
      }
      setOpacity(String(Number(moved.alpha.toFixed(2))));
      if (!visible) {
        // This frame makes the note visible; the next one writes the text.
        visible = true;
        return;
      }
      if (moved.seq !== textSeq) {
        textSeq = moved.seq;
        note.textContent = moved.text;
        // An edit between two frames can have ended the app's hold (a rename) and made this report: the pointer is still
        // over the note, so ask again. Asking while held is a no-op in the app.
        if (held) app.set_moved_hold(true);
      }
    },
    reset: () => {
      summary.reset();
      dragNote.reset();
      showButton(false);
      hide();
    },
  };
}
