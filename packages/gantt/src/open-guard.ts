// What `Gantt.open` and `Gantt.load` share (gs#1152): one token, so the file started last is the one that lands, and one check
// right before `App.open`, so a drag, a cell edit or an edit made while the file was read is never replaced.

import type { Say } from "./messages.js";

/** A replacement that has started: its place in the order, and whether the schedule had unsaved changes when it began. */
export interface OpenTurn {
  readonly id: number;
  readonly unsaved: boolean;
}

export interface OpenGuard {
  /** The chart's sentence for `key`, in the host's language (`Gantt.load` words its refusals with it). */
  say: Say;
  /** Why a replacement cannot start now (a sentence), or null. */
  busy(): string | null;
  /** Start a replacement; any one still reading is now stale. */
  take(): OpenTurn;
  /** A newer replacement started after `turn`. */
  stale(turn: OpenTurn): boolean;
  /** Right before `App.open`: why the swap must not happen (a sentence), or null to go ahead. */
  refusal(turn: OpenTurn): string | null;
}

/** `busy` is `Gantt.busy()`; `unsaved` reads the unsaved mark now, not at the last frame. */
export function createOpenGuard(state: { busy: () => boolean; unsaved: () => boolean; say: Say }): OpenGuard {
  let latest = 0;
  const { say } = state;
  return {
    say,
    busy: () => (state.busy() ? say("open-busy") : null),
    take: () => ({ id: ++latest, unsaved: state.unsaved() }),
    stale: (turn) => turn.id !== latest,
    refusal: (turn) => (state.busy() ? say("open-busy") : !turn.unsaved && state.unsaved() ? say("edited") : null),
  };
}
