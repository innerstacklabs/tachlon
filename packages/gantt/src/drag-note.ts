import { type Status } from "./api.js";
import { listening, type Shell } from "./shell.js";

export interface DragNote {
  /** Show the app's note for a drag that moved nothing, or hide it; write only what changed. */
  sync: (s: Status) => void;
  /** A new schedule was opened: hide the note. */
  reset: () => void;
}

/** How long the note stays, in milliseconds, if no press ends it first. */
export const NOTE_MS = 6000;

/**
 * The drag note: a drag on a finished or started task moves nothing, and the line says why (`Session::drag_note`), in the same dark chip as
 * the baseline summary, above it at the canvas's bottom right. It is information, not an error, so it is not the red alert line, and it
 * covers no table header. The app clears it with the next press; the page also hides it after {@link NOTE_MS}, from one timer set at the later
 * of the text's arrival and the release of the press (a started task's text arrives on the first refused frame, with the button still down),
 * so an idle frame compares one string and writes nothing.
 */
export function installDragNote(shell: Shell): DragNote {
  const { root } = shell;
  const chip = root.querySelector<HTMLDivElement>("[data-part=drag-note]");
  if (chip === null) throw new Error("missing #drag-note");
  /** The text the app last reported, and the text on show (null once the timer hid it). */
  let reported: string | null = null;
  let shown: string | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** A press is down: the note stays until it ends. */
  let down = false;
  const show = (text: string | null): void => {
    if (text === shown) return;
    shown = text;
    chip.hidden = text === null;
    chip.textContent = text ?? "";
  };
  const stop = (): void => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const arm = (): void => {
    stop();
    timer = setTimeout(() => show(null), NOTE_MS);
  };
  // On the host's element: `listening` takes them off when the chart goes.
  listening(shell, root, "pointerdown", () => {
    down = true;
  });
  const release = (): void => {
    down = false;
    if (shown !== null) arm();
  };
  listening(shell, root, "pointerup", release);
  listening(shell, root, "pointercancel", release);
  return {
    sync: (s) => {
      const note = s.link.note;
      if (note === reported) return;
      reported = note;
      stop();
      show(note);
      if (note !== null && !down) arm();
    },
    reset: () => {
      reported = null;
      stop();
      show(null);
    },
  };
}
