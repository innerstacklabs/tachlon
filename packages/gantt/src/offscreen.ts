import { type Status } from "./api.js";
import { nameParts } from "./dom.js";
import { type Shell } from "./shell.js";
import "./styles/offscreen.css";

export interface Offscreen {
  /** Show, word and place the edge note for `status.offscreen`; write only what changed. */
  sync: (s: Status) => void;
}

/** The note's words for the nearest earlier finish `before` and the nearest later start `after`; at least one is a date. */
export function offscreenText(before: string | null, after: string | null): string {
  if (before !== null && after !== null) return `no work ${before} to ${after}`;
  if (before !== null) return `Work on these rows: earlier, ${before}`;
  return `Work on these rows: later, ${after ?? ""}`;
}

/**
 * The edge note (gs#309e1): when no row in the window has work in its time span, one note says where the work went, at the left
 * edge when only earlier work exists, at the right when only later work does, and in the middle when both do. A click calls
 * `App.go_to_offscreen`, which pans there (the later work when both exist) and keeps the rows. The words and the dates come from
 * the app (`status.offscreen`); the page only shows them, and writes only when they change.
 *
 * The note takes no focus: a press on it is not defaulted, so the chart keeps the keys and the focus ring where they were. A wheel
 * or a pinch over it goes on to the canvas as a copy of the same event, so the note never stops a pan or a zoom.
 */
export function installOffscreen({ root, canvas, app }: Shell): Offscreen {
  const note = document.createElement("button");
  note.dataset.part = "offscreen-note";
  nameParts(root, note);
  note.type = "button";
  note.tabIndex = -1;
  note.hidden = true;
  // Before the legend strip, which stays the chart's last child; appended when the page has no legend.
  root.insertBefore(note, root.querySelector(":scope > [data-part=legend]"));
  let shown: string | null = null;
  let side = "";
  /** Whether a click goes to the later work: true unless only earlier work exists. */
  let later = true;
  note.addEventListener("mousedown", (e) => e.preventDefault());
  note.addEventListener("click", () => {
    app.go_to_offscreen(later);
  });
  note.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      canvas.dispatchEvent(new WheelEvent("wheel", e));
    },
    { passive: false },
  );
  return {
    sync: (s) => {
      const o = s.offscreen;
      const text = o === null ? null : offscreenText(o.b, o.a);
      if (text === shown) return;
      shown = text;
      note.hidden = text === null;
      if (o === null || text === null) return;
      note.textContent = text;
      later = o.a !== null;
      const next = o.b !== null && o.a !== null ? "both" : o.b !== null ? "before" : "after";
      if (next !== side) {
        side = next;
        note.dataset.side = next;
      }
    },
  };
}
