import type { HostInit } from "../src/measure/config";
import { LinkEditSteps } from "../src/measure/link-edit-phase";
import { expect, test } from "./support/gantt";

// The link-edit phase of the measurement (gs#725) on a stub host, no page. The smoke run in `measure.spec.ts` covers the same
// through the real pointer path and the real engine; this one fails fast and names the step.

/** Frames the stub's slide reports active after an edit. */
const ACTIVE_READS = 4;

interface Stub {
  /** Edits in order: "add" or "remove"; `at` is how many of them are applied. */
  history: ("add" | "remove")[];
  at: number;
  pressed: boolean;
  activeLeft: number;
}

const linked = (s: Stub): boolean => s.history[s.at - 1] === "add";

/**
 * A host with three leaf rows on screen. An Alt-drag from row 2 onto row 0 adds a link that moves rows 0 and 1; every other
 * drag leaves the revision as it was, as the engine's refusals of a loop or a duplicate do.
 */
function stubHost(s: Stub): HostInit {
  const rowHeight = 20;
  const drag = { from: -1, to: -1 };
  const edit = (op: "add" | "remove"): void => {
    s.history.length = s.at;
    s.history.push(op);
    s.at++;
    s.activeLeft = ACTIVE_READS;
  };
  const step = (by: number): boolean => {
    const next = s.at + by;
    if (next < 0 || next > s.history.length) return false;
    s.at = next;
    s.activeLeft = ACTIVE_READS;
    return true;
  };
  const host = {
    app: {
      table_rows: () => JSON.stringify([1, 2, 3].map((id) => [id, 0, 0, 0, 0, 0])),
      row_start: (row: number) => (linked(s) && row < 2 ? 1000 + row : row * 10),
      x_of: (t: number) => t / 100,
      selected_link: () => (linked(s) ? 7 : undefined),
      remove_selected_link: () => {
        edit("remove");
        return 7;
      },
      undo: () => step(-1),
      redo: () => step(1),
    },
    canvas: { getBoundingClientRect: () => ({ width: 800, height: 600 }) },
    input: {
      move: (_x: number, y: number) => {
        if (s.pressed) drag.to = Math.floor(y / rowHeight);
      },
      down: (_x: number, y: number, alt: boolean) => {
        s.pressed = alt;
        drag.from = drag.to = Math.floor(y / rowHeight);
      },
      up: () => {
        if (s.pressed && drag.from === 2 && drag.to === 0) edit("add");
        s.pressed = false;
      },
    },
    status: () => ({
      n: 3,
      axisHeight: 0,
      rowHeight,
      scrollY: 0,
      revision: s.at,
      get slide() {
        const active = s.activeLeft > 0;
        if (active) s.activeLeft--;
        return { active, copyMs: 0.25 };
      },
    }),
  };
  return host as unknown as HostInit;
}

class StubLinkEdit extends LinkEditSteps {
  progress = "";

  constructor(state: Stub) {
    super(stubHost(state));
  }

  /** One recorded link edit; real time, about 3 s of windows. */
  runOnce(): void {
    const run = this.linkEditRun({ warmup: 0, editRepeats: 1 } as Parameters<LinkEditSteps["linkEditRun"]>[0]);
    while (!run.next().done);
  }

  result() {
    return this.linkEditResult(16.7);
  }
}

test("the link-edit phase picks the pair the engine accepts, then adds, removes, undoes and redoes it", () => {
  const state: Stub = { history: [], at: 0, pressed: false, activeLeft: 0 };
  const steps = new StubLinkEdit(state);
  steps.runOnce();
  const result = steps.result();
  expect(result?.task, "the successor is the first row's task").toBe(1);
  expect(result?.pred, "the predecessor is the last row's task").toBe(3);
  expect(result?.movedPerRepeat, "the add moved the successor and the row after it").toEqual([2]);
  expect(result?.refused, "the five other pairs are refused and skipped").toBe(5);
  expect(result?.candidates.length, "every pair was tried").toBe(6);
  expect(result?.copyMs?.count, "one copy time per recorded edit").toBe(1);
  expect(result?.setMs.count, "one release time per recorded edit").toBe(1);
  expect(linked(state), "the repeat ends with the link removed, as it began").toBe(false);
  // The trial add was undone (the cursor moved back and the next edit replaced it), so the history holds the repeat's own edits.
  expect(state.history, "the stub saw the add and the removal").toEqual(["add", "remove"]);
});

test("the link-edit phase fails when no pair on screen can be linked", () => {
  const state: Stub = { history: [], at: 0, pressed: false, activeLeft: 0 };
  const steps = new StubLinkEdit(state);
  // Alt is never down, so every drop is refused: a press without Alt is a bar drag, which the stub ignores.
  const refuse = (steps as unknown as { host: { input: { down: (x: number, y: number, alt: boolean) => void } } }).host.input;
  const down = refuse.down;
  refuse.down = (x, y) => down(x, y, false);
  expect(() => steps.runOnce()).toThrow("no pair of bars on screen moves any row");
});
