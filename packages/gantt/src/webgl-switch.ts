// The switch to WebGL2 inside the chart's own root (gs#1217). A WebGPU canvas that failed cannot take a WebGL2 context, so the chart takes
// down its canvas and renderer and starts again in the same root on a fresh canvas, forced to WebGL2, with the schedule it held. The
// page around it is never reloaded: the host keeps its state, and hears it once, through `on("error")` with the source `backend`. The
// new start replaces the root's children, so anything a host put inside them goes with the old ones. A panic leaves the WASM module
// unusable on this page, so it cannot switch: it is reported as a `fatal` error, the chart shows its message, and the page that holds the
// chart decides whether to reload. Nothing here runs on a frame that draws.
import { reason } from "./boot.js";
import { createEvents, type EventReads, type Events } from "./events.js";
import { unusable } from "./frame-guard.js";
import type { Gantt, MountOptions } from "./index.js";
import { MountError, type StartSource } from "./mount-load.js";
import type { PublicApp } from "./api.js";
import { type Shell, takeDown } from "./shell.js";
import { claimIds } from "./dom.js";
import type { HookRecords } from "./test-hook.js";
import type { OpenedFile } from "./open-schedule.js";
import { last_panic } from "./wasm/app.js";
import { refused } from "./edit-result.js";

/** How one start of the chart runs. */
export interface Run {
  forceWebgl: boolean;
  /** Why WebGPU was abandoned for WebGL2, or null if it was not. */
  fallback: string | null;
  /** The events, shared by every start, so a host's handlers outlive a switch. */
  events: Events;
  /** Give the events this start's readers: its view state (`change` compares the states it hands back), its commits and its task keys, from now on. */
  reads: (reads: EventReads) => void;
  /** Called by the loop when a WebGPU frame fails; see `LoopParts.lost`. */
  lost: (err: unknown) => boolean;
  /** The first attempt: it arms the developer page's test panic (`?panic=init`). */
  first: boolean;
  /** The schedule holds changes no save holds yet (carried over by a switch): the unsaved mark starts on. */
  unsaved: boolean;
  /** The file the schedule came from (carried over by a switch): the start adopts it again, for its notes and its test hook. */
  file: FileRecord | null;
  /** Whether the host has the chart read-only now: the start passes it to its app as the app is made. */
  readOnly: () => boolean;
}

/** A file the chart opened: what was read, the importer's notes, and the notes the host said to leave out of the toolbar. */
export interface FileRecord {
  file: OpenedFile;
  lines: string[];
  expected: readonly string[];
  /** What `ViewState.fileName` reads: the file's name, null for a schedule loaded from a document. */
  fileName: string | null;
}

/** One running chart: what the host drives, and whether its schedule is still the one it started on. */
export interface Started {
  gantt: Gantt;
  /** A file was opened, or the schedule was edited, since the start. */
  changed(): boolean;
  /** The unsaved mark is on. */
  unsaved(): boolean;
  /** The file the schedule came from, if one was opened or carried over. */
  file(): FileRecord | null;
  /** This start's test hook (`window.gantt`): what the host keeps is recorded on it, and it is taken down at a switch. */
  hook: HookRecords;
}

/** What one start owns: what `destroy` runs, last registered first (shell.ts), and its app once it is made. */
export interface Own {
  disposers: Shell["disposers"];
  app: PublicApp | null;
}

/** Run one start; if it throws part-way, take down what it put up and free its app before the throw goes on. */
export async function startTakingDown(go: (own: Own) => Promise<Started>): Promise<Started> {
  const own: Own = { disposers: [], app: null };
  try {
    return await go(own);
  } catch (err) {
    takeDown(own.disposers, own.app === null ? undefined : () => own.app?.free());
    throw err;
  }
}

/** The commands that answer an `EditResult`, a `RowResult`, a `WhatIfResult`, an `ExplainResult`, a `HealthResult`, a `ProposeResult` or an `AcceptResult`: during a switch they refuse with `"busy"`. */
const EDITS = new Set<string | symbol>(["insertRow", "deleteRow", "assignKey", "deleteLink", "setDataDate", "setBaseline", "clearBaseline", "addNewToBaseline", "whatIf", "whatIfLonger", "explain", "health", "criticalPath", "propose", "acceptProposal", "acceptProposed"]);

/** Start the chart once in the root, on `source`. Rejects with a `MountError`, as `mount` does. */
export type Start = (source: StartSource, run: Run) => Promise<Started>;

/** How many times the WebGL2 start of a switch is tried, and the wait between tries in ms. */
const SWITCH_TRIES = 3;
const SWITCH_WAIT = 100;

/**
 * The WebGL2 start of a switch. Right after a WebGPU chart is taken down, `getContext("webgl2")` can return null once (SwiftShader, with
 * a sibling chart on the shared device) and succeed on the next try, so a render failure is tried again before the switch gives up.
 * Any other failure, and a panic, is the caller's at once.
 */
async function startWebgl(start: Start, source: StartSource, run: Run): Promise<Started> {
  for (let tries = 1; ; tries++) {
    try {
      return await start(source, run);
    } catch (err) {
      if (tries >= SWITCH_TRIES || !(err instanceof MountError) || err.stage !== "render" || last_panic() != null) throw err;
      console.warn(`gantt: the WebGL2 start failed, trying again: ${err.message}`);
      await new Promise((resolve) => setTimeout(resolve, SWITCH_WAIT));
    }
  }
}

/**
 * Start the chart and keep it drawing: a WebGPU start that fails without a panic starts again on WebGL2, and so does a WebGPU frame
 * that fails later. `restart` is the developer page's own restart, for a panic; a host page gives none. Returns the `Gantt` the host
 * keeps, which drives whichever start is current.
 */
export async function mountSwitching( // oxlint-disable-line max-lines-per-function -- existing, over cap
  root: HTMLDivElement,
  options: Omit<MountOptions, "tasks" | "links">,
  start: Start,
  pageFallback: string | null,
  restart: ((why: string) => void) | undefined,
): Promise<Gantt> {
  // The events read through the current start's readers, which each start hands over before its loop starts.
  let readers: EventReads | null = null;
  const events = createEvents(() => {
    if (readers === null) throw new Error("no start has given the events its readers");
    return readers;
  });
  const reads = (next: EventReads): void => {
    readers = next;
  };
  const forced = options.forceWebgl === true;
  // What the host last asked for, read by each start as it makes its app, so a switch to WebGL2 and a call during one both keep it.
  let readOnly = options.readOnly === true;
  const readOnlyNow = (): boolean => readOnly;
  let current: Started;
  let switching: Promise<void> | null = null;
  let failed: string | null = null;
  let destroyed = false;
  // The document the switch carries, which `toDocument` returns while the new start is not up yet.
  let kept: unknown = null;
  // The `open`, `save` and `load` calls still running on the current start: a switch lets them end before it keeps the schedule.
  const running = new Set<Promise<unknown>>();
  const tracked = <T>(call: Promise<T>): Promise<T> => {
    running.add(call);
    void call.finally(() => running.delete(call)).catch(() => undefined);
    return call;
  };

  // The `error` event goes out on the next animation frame, once: a host that adds its handler when `mount` resolves hears it.
  const announce = (message: string): void => {
    requestAnimationFrame(() => {
      if (!destroyed) events.fail("backend", message, false);
    });
  };

  const switchToWebgl = async (why: string): Promise<void> => { // oxlint-disable-line max-lines-per-function -- existing, over cap
    // Out of the frame that failed: the old chart's loop has ended, and it is taken down here, once a file it was opening is in.
    await Promise.resolve();
    await Promise.allSettled([...running]);
    const old = current;
    let source: StartSource = options.schedule;
    // The document is kept when the schedule has changes no save holds (a baseline set or cleared moves no revision, but the mark
    // sees it), differs from what `mount` was given, or was given as `tasks` and `links`, which `mount` lets go once it has started. A
    // state that cannot be read counts as changed, so the document is tried anyway.
    const read = (what: () => boolean): boolean => {
      try {
        return what();
      } catch (err) {
        console.error("the chart's state could not be read before the switch", err);
        return true;
      }
    };
    const unsaved = read(() => old.unsaved());
    const changed = read(() => old.changed());
    let lostEdits = "";
    try {
      kept = old.gantt.toDocument();
      if (unsaved || changed || source === undefined) source = { document: kept };
    } catch (err) {
      console.error("the schedule could not be kept for WebGL2", err);
      if (unsaved || changed) {
        lostEdits = ` The schedule could not be kept, so it starts again from what was loaded and the changes since were lost (${reason(err)}).`;
      }
    }
    // The file the schedule came from: the new start adopts it again, for its notes, its name and its test hook.
    let file: FileRecord | null = null;
    try {
      file = old.file();
    } catch (err) {
      console.error("the opened file could not be read before the switch", err);
    }
    old.hook.drop();
    // The chart's number is held from the old start's teardown to the new start's claim, so the chart keeps its ids.
    const holdIds = claimIds(root);
    old.gantt.destroy();
    if (destroyed) {
      holdIds();
      return;
    }
    // The old start's loop has stopped: the new start's first status is compared with nothing, and its view state fires one `change`.
    events.restart();
    try {
      current = await startWebgl(start, source, { forceWebgl: true, fallback: why, events, reads, lost, first: false, unsaved, file, readOnly: readOnlyNow });
      kept = null;
      if (destroyed) {
        current.gantt.destroy();
        return;
      }
      announce(`WebGPU failed, so the chart draws with WebGL2: ${why}.${lostEdits}`);
      current.hook.api(gantt);
    } catch (err) {
      failed = `WebGPU failed (${why}), and WebGL2 could not start: ${err instanceof Error ? err.message : String(err)}`;
      root.replaceChildren();
      const line = document.createElement("div");
      line.setAttribute("role", "alert");
      line.textContent = failed;
      root.append(line);
      // Not the switch: the chart is gone, and only a reload of the page can bring it back.
      events.fail("backend", failed, true);
    } finally {
      holdIds();
    }
  };

  function lost(err: unknown): boolean {
    if (switching !== null) return true;
    const why = reason(err);
    console.warn(`gantt: WebGPU failed: ${why}`);
    if (unusable(err)) {
      if (restart === undefined) return false;
      restart(why);
      return true;
    }
    switching = switchToWebgl(why).finally(() => {
      switching = null;
    });
    return true;
  }

  try {
    current = await start(options.schedule, { forceWebgl: forced, fallback: pageFallback, events, reads, lost, first: true, unsaved: false, file: null, readOnly: readOnlyNow });
  } catch (err) {
    // A WebGPU start that failed without a panic starts again on WebGL2; a panic, or a WebGL2 start that failed, is the caller's.
    // `last_panic` is asked last: it needs the engine, which a `"engine"` failure never loaded.
    if (forced || !(err instanceof MountError) || err.stage !== "render" || last_panic() != null) throw err;
    console.warn(`gantt: WebGPU failed to start: ${err.message}`);
    current = await start(options.schedule, { forceWebgl: true, fallback: err.message, events, reads, lost, first: false, unsaved: false, file: null, readOnly: readOnlyNow });
    announce(`WebGPU failed, so the chart draws with WebGL2: ${err.message}`);
  }

  // Each call goes to the chart that is current. During a switch the async calls wait for it, `toDocument` returns the document the
  // switch carries, `busy` is true, and the others throw.
  const now = (): Gantt => {
    if (failed !== null && !destroyed) throw new Error(failed);
    if (switching !== null && !destroyed) throw new Error("the chart is switching to WebGL2");
    return current.gantt;
  };
  const settled = async (): Promise<Gantt> => {
    if (switching !== null) await switching;
    return now();
  };
  const gantt: Gantt = {
    // Each command looks up the current start's at call time, so one a host took out of `commands` before a switch still drives the chart.
    commands: new Proxy({} as Gantt["commands"], {
      get: (_, key) => {
        const value = (current.gantt.commands as unknown as Record<string | symbol, unknown>)[key];
        if (typeof value !== "function") return value;
        return (...args: unknown[]): unknown => {
          // While the chart switches it takes no command: the edit commands refuse with `"busy"` and its sentence, as `busy()` says, the rest give false.
          if (switching !== null && !destroyed) return key === "setBaselineFromFile" ? Promise.resolve(refused("busy")) : EDITS.has(key) ? refused("busy") : false;
          const commands = current.gantt.commands as unknown as Record<string | symbol, (...a: unknown[]) => unknown>;
          return commands[key]!.apply(commands, args);
        };
      },
    }),
    open: async (file, opts) => tracked((await settled()).open(file, opts)),
    save: async () => tracked((await settled()).save()),
    load: async (source) => tracked((await settled()).load(source)),
    newSchedule: async () => tracked((await settled()).newSchedule()),
    exportCsv: () => now().exportCsv(),
    exportXlsx: () => now().exportXlsx(),
    copyList: () => now().copyList(),
    exportImage: (options) => now().exportImage(options),
    printedPages: () => now().printedPages(),
    printOn: (size) => now().printOn(size),
    // During a switch: the old start's document while it is still up (a running open, save or load is ending), then the one kept,
    // which a switch whose WebGL2 start failed still returns, so a host can save the edits before it reloads.
    toDocument: () => {
      if (switching !== null && !destroyed) return kept ?? current.gantt.toDocument();
      if (failed !== null && kept !== null && !destroyed) return kept;
      return now().toDocument();
    },
    snapshot: () => now().snapshot(),
    exportXer: () => now().exportXer(),
    exportMspdi: () => now().exportMspdi(),
    checks: () => now().checks(),
    backend: () => now().backend(),
    state: () => now().state(),
    filterInputs: () => now().filterInputs(),
    speed: () => now().speed(),
    setPending: (keys) => now().setPending(keys),
    // A call during a switch is kept for the start that follows it; the others throw then.
    setReadOnly: (on) => {
      if (switching === null || destroyed) now().setReadOnly(on);
      readOnly = on;
    },
    readOnly: () => readOnly,
    // While the chart switches it can take no edit: busy, as a host that asks before an edit command would want.
    busy: () => (switching !== null && !destroyed ? true : now().busy()),
    on: (name, handler) => {
      if (destroyed) throw new Error("this chart was destroyed");
      return events.on(name, handler);
    },
    destroy: () => {
      if (destroyed) return;
      destroyed = true;
      current.gantt.destroy();
      if (failed !== null) root.replaceChildren();
    },
  };
  current.hook.api(gantt);
  root.dispatchEvent(new CustomEvent("gantt:ready", { detail: gantt.commands, bubbles: true }));
  return gantt;
}
