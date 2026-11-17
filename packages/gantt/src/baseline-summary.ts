import { type Status } from "./api.js";
import { createBaselineClauses } from "./baseline-text.js";
import { type Shell } from "./shell.js";

export interface BaselineSummaryChip {
  /** Show how the schedule differs from the baseline, or hide the chip with none; write only what changed. */
  sync: (s: Status) => void;
  /** A new schedule was opened: hide the chip. */
  reset: () => void;
}

/**
 * The baseline summary: one line at the canvas's bottom right, `vs baseline: 46 slipped · finish +26 days · critical 0
 * joined, 3 left`, shown while a baseline is set. Its own `createBaselineClauses` hands back the same array while the counts stand, so the
 * chip builds and writes its text only when the array is a new one: an edit, a drag (the app recomputes it at most every
 * 150 ms) and an undo each update it, and an idle frame builds no string and writes nothing. It is not a live region: it
 * changes with every drag step, and the "what moved" note is the one that announces.
 */
export function installBaselineSummary({ root, fmt }: Shell): BaselineSummaryChip {
  const chip = root.querySelector<HTMLDivElement>("[data-part=baseline-summary]");
  if (chip === null) throw new Error("missing #baseline-summary");
  const baselineClauses = createBaselineClauses();
  let shown: string | null = null;
  let seen: readonly string[] | null = null;
  const show = (text: string | null): void => {
    if (text === shown) return;
    shown = text;
    chip.hidden = text === null;
    chip.textContent = text ?? "";
  };
  return {
    sync: (s) => {
      const clauses = baselineClauses(s.baseline, fmt);
      if (clauses === seen) return;
      seen = clauses;
      show(clauses === null ? null : `vs baseline: ${clauses.join(" · ")}`);
    },
    reset: () => {
      seen = null;
      show(null);
    },
  };
}
