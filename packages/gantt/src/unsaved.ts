import { type BaselineSummary, type Status } from "./api.js";

export interface Unsaved {
  /** Per frame: compare the status with the save point; `changed` is told when the answer changes. */
  sync: (s: Status) => void;
  /** The schedule was saved as of `s` (read before the document was written, so an edit during the write stays unsaved). */
  saved: (s: Status) => void;
  /** The schedule just opened holds work no save holds (`s` read after the open), so it shows as unsaved until a save or the next open. */
  carry: (s: Status) => void;
}

/**
 * Whether two baseline readings are the same. Setting or clearing the baseline does not move `Status.revision`, so the
 * summary stands in for it: a set or clear changes `null` or the counts. Setting a baseline over one that reads the same
 * is not seen.
 */
function sameBaseline(a: BaselineSummary | null, b: BaselineSummary | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.slipped === b.slipped &&
    a.finish_days === b.finish_days &&
    a.joined === b.joined &&
    a.left === b.left &&
    a.added === b.added &&
    a.removed === b.removed
  );
}

/**
 * Whether the schedule has changed since the last save or open (`ViewState.unsaved`; the app's chrome shows the mark beside the file name).
 * Undo and redo are changes. `changed` is told each time the answer changes. `unsaved` starts the chart with changes no save holds yet (the
 * edits a switch to WebGL2 carried over): the answer stays until a save or an open.
 */
export function installUnsaved(s0: Status, changed?: (unsaved: boolean) => void, unsaved = false): Unsaved {
  let epoch = s0.epoch;
  let at = s0;
  let carried = unsaved;
  let shown = false;
  const show = (on: boolean): void => {
    if (on === shown) return;
    shown = on;
    changed?.(on);
  };
  return {
    sync: (s) => {
      // A file opened in place is the new save point.
      if (s.epoch !== epoch) {
        epoch = s.epoch;
        at = s;
        carried = false;
      }
      show(carried || s.revision !== at.revision || !sameBaseline(s.baseline, at.baseline));
    },
    carry: (s) => {
      epoch = s.epoch;
      at = s;
      carried = true;
    },
    saved: (s) => {
      at = s;
      carried = false;
    },
  };
}
