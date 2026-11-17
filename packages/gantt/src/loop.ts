import { type Status } from "./api.js";
import { createBarSpeech } from "./bar-speech.js";
import { reason } from "./boot.js";
import { type HoverCard } from "./card.js";
import { type CellEdit } from "./cell-edit.js";
import { type Events } from "./events.js";
import { createFrameGuard } from "./frame-guard.js";
import { type Metrics } from "./measure.js";
import { type MovedNote } from "./moved.js";
import { installOffscreen } from "./offscreen.js";
import { type SelectionCount } from "./selection-count.js";
import { type OpenControls } from "./open-schedule.js";
import { type PointerInput } from "./pointer.js";
import { type Unsaved } from "./unsaved.js";
import { listening, type Shell } from "./shell.js";
import { type Table } from "./table.js";
import { type TableAria } from "./table-aria.js";
import { type IdColumn } from "./id-column.js";

/** A part the page puts beside the chart (its toolbar, its find box): it follows each frame's status. The loop never names one. */
export interface Beside {
  sync: (s: Status) => void;
}

export interface LoopParts {
  /** What a host listens to through `Gantt.on`; the loop feeds it each frame's `Status`. It outlives this loop when the chart starts again on WebGL2. */
  events: Events;
  metrics: Metrics;
  /**
   * A frame failed on an automatic WebGPU choice: true when the entry takes it over (the chart starts again on WebGL2, or the page
   * restarts), and the loop ends. Null on WebGL2, where a failed frame is reported and retried.
   */
  lost: ((err: unknown) => boolean) | null;
  /** The page's restart, which the frame banner's Reload button calls; a host page has none. */
  reload: (() => void) | undefined;
  /** The `Status.epoch` the shell's state was built for. */
  epoch: number;
  overlay: HTMLDivElement;
  statusLine: HTMLDivElement | null;
  statusText: (s: Status, lines: string[]) => string;
  fileOpener: OpenControls;
  /** Synced each frame in this order, after the events. They reset for a new file through `on("open")`. */
  beside: readonly Beside[];
  idColumn: IdColumn;
  table: Table;
  tableAria: TableAria;
  edit: CellEdit;
  card: HoverCard;
  moved: MovedNote;
  selectionCount: SelectionCount;
  pointer: PointerInput;
  saveState: Unsaved;
}

/** How long the loop keeps drawing after the last change: the engine's own timed motions (the path reveal runs 300 ms) end inside it. */
const QUIET_MS = 500;
/** `Status.frames` counts the loop's own draws, so a comparison of two statuses leaves it out. */
const FRAMES = /"frames":\d+/;
/** The `App` methods that do not wake the loop: it draws, and the app is freed once. */
const OWN = new Set(["constructor", "free", "frame", "__destroy_into_raw"]);

/**
 * The requestAnimationFrame loop: one `App.frame` per tick, then the DOM follows the status. The loop draws while something changes and
 * for `QUIET_MS` after, then stops asking for frames; any call into the app from outside a tick (input, a resize, a host command, a
 * read) wakes it. Destroying the chart cancels the pending tick and ends the loop.
 */
export function startLoop(shell: Shell, parts: LoopParts): void { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { app, canvas, root, status } = shell;
  const { metrics, lost, overlay, statusLine, statusText, fileOpener } = parts;
  const { events, beside, idColumn, table, tableAria, edit, card, moved, selectionCount, pointer, saveState } = parts;
  // A file was opened in place: everything this shell keeps for the old schedule goes. Runs once per `Status.epoch` change.
  let epoch = parts.epoch;
  const resetForNewSchedule = (): void => {
    edit.end();
    idColumn.show();
    table.reset();
    card.reset();
    moved.reset();
    pointer.forget();
    canvas.classList.remove("dragging", "held", "linking", "refused", "resize");
  };
  const offscreen = installOffscreen(shell);
  const speakSelection = createBarSpeech(shell);
  // The status line is written when its text changed: assigning textContent replaces the text node even with the same text.
  let lineShown: string | null = null;
  let last = performance.now();
  let interval = 0;
  let stopped = false;
  // `pending` is the requested tick's id, 0 while the loop sleeps; `changed` is when the app last changed or was called into.
  let pending = 0;
  let changed = last;
  let drawn = "";
  let ticking = false;
  const stop = (): void => {
    stopped = true;
    cancelAnimationFrame(pending);
  };
  const wake = (): void => {
    if (ticking || stopped) return;
    changed = performance.now();
    if (pending === 0) {
      last = changed;
      pending = requestAnimationFrame(tick);
    }
  };
  // Every other part of the page holds this app object, so the wake goes on it: a call from outside a tick may change what a frame draws.
  const target = app as unknown as Record<string, unknown>;
  for (const name of Object.getOwnPropertyNames(Object.getPrototypeOf(app) as object)) {
    const method = target[name];
    if (OWN.has(name) || typeof method !== "function") continue;
    target[name] = (...args: unknown[]): unknown => {
      wake();
      return (method as (...a: unknown[]) => unknown).apply(app, args);
    };
  }
  listening(shell, document, "visibilitychange", wake);
  // The engine reports a lost GPU device on the window, from the browser and not from a call: the next frame fails and the chart moves to WebGL2.
  listening(shell, window, "gpulost", wake);
  // Another chart's panic stops this one too (the module is shared), and this chart reports it as its own frame error.
  const frameGuard = createFrameGuard({
    root,
    describe: reason,
    ...(parts.reload ? { reload: parts.reload } : {}),
    // Stopped for good: only a reload of the page brings it back.
    halt: (why) => {
      stop();
      events.fail("frame", why, true);
    },
  });
  shell.disposers.push(() => {
    frameGuard.dispose();
    stop();
  });
  const tick = (now: number): void => {
    pending = 0;
    if (stopped) return;
    ticking = true;
    interval = now - last;
    last = now;
    metrics.begin(now, interval);
    const encodeStart = performance.now();
    if (!frameGuard.run(
      () => app.frame(),
      (err) => {
        if (lost !== null && lost(err)) return true;
        events.fail("frame", reason(err), frameGuard.stops(err));
        return false;
      },
      tick,
    )) {
      ticking = false;
      return;
    }
    const encodeMs = performance.now() - encodeStart;
    const s = status();
    if (s.epoch !== epoch) {
      epoch = s.epoch;
      resetForNewSchedule();
    }
    events.watch(s);
    fileOpener.frame(s.frames);
    for (const part of beside) part.sync(s);
    saveState.sync(s);
    table.sync(s);
    tableAria.sync(s);
    card.sync(s);
    speakSelection(s);
    moved.sync(s);
    offscreen.sync(s);
    selectionCount.sync(s);
    metrics.end(interval, encodeMs, s);
    canvas.classList.toggle("held", !!s.drag?.active && s.drag.held);
    pointer.sync(s);
    // The overlay is the page's message line: it shows a measured run's progress and nothing else while the app runs.
    const lines = metrics.lines();
    const run = lines.find((l) => l.startsWith("measure ")) ?? "";
    if (overlay.textContent !== run) overlay.textContent = run;
    if (statusLine !== null && !statusLine.hidden) {
      const text = statusText(s, lines);
      if (text !== lineShown) {
        statusLine.textContent = text;
        lineShown = text;
      }
    }
    // What the parts above did to the app belongs to this frame's state: a change there, or a motion that runs, asks for the next frame.
    const state = app.status().replace(FRAMES, "");
    if (state !== drawn) {
      drawn = state;
      changed = now;
    }
    ticking = false;
    if (now - changed < QUIET_MS || metrics.script !== null || s.drag !== null || s.link.drag !== null || s.slide.active || (s.moved?.alpha ?? 0) > 0) {
      pending = requestAnimationFrame(tick);
    }
  };
  pending = requestAnimationFrame(tick);
}
