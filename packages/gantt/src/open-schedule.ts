// Read a schedule file, hand it to the wasm module as bytes and swap it in with App.open (`Gantt.open`). Nothing here touches the page beyond the
// chart's root: the Open button, the file chooser, drops, the project list and the error line are the app's chrome (`apps/chrome`).
// Nothing here makes a network request.
import type { PublicApp } from "./api.js";
import { documentText } from "./document.js";
import { say, type Vars } from "./messages.js";
import type { OpenGuard } from "./open-guard.js";
import { PRODUCT_NAME } from "./product.js";
import { LoadedSchedule } from "./wasm/app.js";
import type { MountOptions } from "./index.js";

/** The largest file the page reads. A larger one gets a message and is not read at all. */
export const MAX_OPEN_BYTES = 256 * 1024 * 1024;

const MIB = 1024 * 1024;

/**
 * Where the time of one open went, in milliseconds on `performance.now()`. The phases are back to back and do
 * not overlap, so `read + paint + copy + parse + build + compute + swap + firstFrame` is `total` (up to the
 * rounding of the clock). `parse`, `build` and `compute` are `LoadedSchedule.timings()`; the rest are the page's.
 * A value that is not a finite number, or is negative, is an invalid measurement: it is kept as it is (NaN stays
 * NaN, a negative stays negative), never turned into 0, and `measure.mjs` rejects the open that holds it.
 */
export interface OpenTimings {
  /** The change or drop event to `file.arrayBuffer()` resolved. */
  read: number;
  /** The wait that lets "Opening…" paint before the parse, which does not yield: one frame and a task. */
  paint: number;
  /**
   * The call to `parse_bytes` minus its three Rust phases: the copy of the bytes into linear memory, building the
   * result, and the clock calls around the phases. It is a remainder, so it would absorb a wrong Rust phase; the
   * check that does not is `rustSpan` against the phases. Not clamped: a negative `copy` is an invalid measurement.
   */
  copy: number;
  /** Detect the format, read the file, map it into the engine schedule (Rust). */
  parse: number;
  /** The display data, the outline and the engine schedule (Rust). */
  build: number;
  /** The solve, the row order and the in-progress date check (Rust). */
  compute: number;
  /** `parse_bytes` returned to `App.open` returned, with the reads of the result in between. */
  swap: number;
  /** `App.open` returned to the first frame submitted after it (the first increment of `Status.frames`); not the display's vblank. */
  firstFrame: number;
  /** The event to the first frame. */
  total: number;
  /**
   * Rust's first to last clock read of `load_bytes`: the same reads that bound the three phases, so it equals
   * `parse + build + compute` up to float rounding however long the process was paused between two reads (gs#248).
   * It is not a phase and not part of `total`, and `measure.mjs` rejects an open where it differs by 3 ms or more.
   */
  rustSpan: number;
}

/** What the page records about a successful open. */
export interface OpenedFile {
  name: string;
  bytes: number;
  /** "xer" or "mspdi", chosen by the file's content, never its name. */
  format: string;
  /** Tasks in the schedule, summary rows included. */
  tasks: number;
  /**
   * Only for a file with several projects: the milliseconds the first pass took (the read and the parse that
   * stopped at the project list). The open that counts starts when the person chooses.
   */
  probeMs?: number;
  /**
   * Set at the first frame after the swap, on this same object, so it is missing for the moment between the swap
   * and that frame (and for good if a newer open replaces this one first).
   */
  timings?: OpenTimings;
}

/** One row of the project list a multi-project XER file carries on its `ChooseProject` error. */
export interface PickerProject {
  /** The P6 `proj_id`, as the text `parse_bytes` takes for `selection`. */
  id: string;
  name: string;
  activities: number;
}

/** The projects of a `ChooseProject` error, or null for any other thrown value. */
export function chooseProjects(err: unknown): PickerProject[] | null {
  if (!(err instanceof Error) || err.name !== "ChooseProject") return null;
  const list = (err as Error & { projects?: unknown }).projects;
  if (!Array.isArray(list)) return null;
  const rows: PickerProject[] = [];
  for (const entry of list as unknown[]) {
    const row = entry as Partial<PickerProject> | null;
    if (typeof row?.id !== "string" || typeof row.activities !== "number") return null;
    rows.push({ id: row.id, name: typeof row.name === "string" ? row.name : "", activities: row.activities });
  }
  return rows.length > 0 ? rows : null;
}

/** @internal */
export interface OpenContext {
  /** The chart root: it gets focus back after an open. */
  root: HTMLElement;
  app: PublicApp;
  /** One line for a thrown value; a Rust panic's message wins. */
  reason: (err: unknown) => string;
  /** The schedule was replaced. The page resets what it keeps for the old one and shows the unsupported list. */
  opened: (file: OpenedFile, unsupported: string[], ms: number, expectedNotes: readonly string[]) => void;
  /** An open did not happen: the `error` event, with the project list when the file holds several projects. */
  fail: (message: string, projects?: PickerProject[]) => void;
  /** `Status.frames` now: frames submitted so far. */
  frames: () => number;
  /** The first frame after the swap was submitted: `file.timings` is set. */
  timed: (file: OpenedFile) => void;
  /** The schedule has changes since the last save or open, read now. An open over them is held until the person answers. */
  unsaved: () => boolean;
  /** What the host stores, read when a file is opened: a file is fitted to it (`LoadedSchedule.conform`) before it is shown. */
  capabilities: () => MountOptions["capabilities"];
  /** The token and the swap check `Gantt.load` shares. */
  guard: OpenGuard;
}

/** What `Gantt.open` takes: the notes the host expects, and the project of a file with several (`"all"` for every one). */
export interface OpenOptions {
  expectedNotes?: readonly string[];
  project?: string;
}

export interface OpenControls {
  /**
   * Open `file`: the size limit, the messages, the project list and the `open` event. Resolves when that open has ended (a file
   * with several projects ends it at the project list, which the `error` event carries); a failure is an `error` event, never thrown.
   */
  openFile(file: File, options?: OpenOptions): Promise<void>;
  /** Call once per frame, after `App.frame()`, with `Status.frames`; it completes the timings of the last open at the first new frame. */
  frame(frames: number): void;
}

/** After the browser has painted the page as it is now. */
function afterPaint(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => setTimeout(resolve, 0));
  });
}

/** @internal */
export function installOpen(ctx: OpenContext): OpenControls { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { app, root } = ctx;
  const words = (key: string, vars?: Vars): string => say(root, key, vars);
  const fail = (key: string, vars: Vars, projects?: PickerProject[]): void => ctx.fail(words(key, vars), projects);
  /** The size limit's refusal: the size shows in MiB from one, else in bytes. */
  const tooBig = (name: string, bytes: number): void =>
    fail("too-big", { name, size: bytes >= MIB ? words("mib", { size: (bytes / MIB).toFixed(1) }) : words("bytes", { count: bytes }), limit: MAX_OPEN_BYTES / MIB });
  // The last successful open, until its first frame is submitted.
  let unframed: { file: OpenedFile; frames: number; swapped: number; started: number; phases: Omit<OpenTimings, "firstFrame" | "total"> } | null =
    null;
  // The file that stopped at a project list, and the time that first pass took: choosing a project reads the file again and records it as `probeMs`.
  let probed: { file: File; ms: number } | null = null;

  /** Read `file`, parse it with `project` (`undefined` lets a one-project file through and stops at a project list) and swap it in. */
  const open = async (file: File, { expectedNotes = [], project }: OpenOptions = {}): Promise<void> => { // oxlint-disable-line max-lines-per-function -- existing, over cap
    // Asked before anything is touched, so a refusal keeps the schedule and its edits. A project choice (`project` set) continues an open already allowed.
    const busy = ctx.guard.busy();
    if (busy !== null) return fail("not-opened", { name: file.name, why: busy });
    if (project === undefined && ctx.unsaved() && !window.confirm(words("open-confirm", { name: file.name }))) return;
    // A newer open or load supersedes this one while it reads: the older one stops when it wakes up.
    const turn = ctx.guard.take();
    const started = performance.now();
    const probeMs = project !== undefined && probed?.file === file ? probed.ms : undefined;
    probed = null;
    // The size is checked before anything is read: a file over the cap is never read whole.
    if (file.size > MAX_OPEN_BYTES) {
      tooBig(file.name, file.size);
      return;
    }
    if (file.size === 0) {
      fail("empty", { name: file.name });
      return;
    }
    let bytes: Uint8Array;
    let read: number;
    try {
      bytes = new Uint8Array(await file.arrayBuffer());
      read = performance.now();
    } catch (err) {
      fail("unreadable", { name: file.name, why: ctx.reason(err) });
      return;
    }
    // Let a host's "Opening…" text paint: the parse that follows runs on the main thread and does not yield.
    await afterPaint();
    if (ctx.guard.stale(turn)) return;
    if (bytes.byteLength > MAX_OPEN_BYTES) {
      tooBig(file.name, bytes.byteLength);
      return;
    }
    // A gzip file that holds a schedule document is read as one; the file's content decides, never its name.
    let text: string | null;
    try {
      text = await documentText(bytes);
    } catch (err) {
      fail("unreadable", { name: file.name, why: ctx.reason(err) });
      return;
    }
    if (ctx.guard.stale(turn)) return;
    let loaded: LoadedSchedule;
    const called = performance.now();
    let returned: number;
    try {
      loaded = text === null ? LoadedSchedule.parse_bytes(bytes, project) : LoadedSchedule.parse_document(text);
      returned = performance.now();
    } catch (err) {
      const projects = project === undefined ? chooseProjects(err) : null;
      if (projects === null) {
        fail("unopenable-format", { name: file.name, why: ctx.reason(err), product: PRODUCT_NAME });
      } else {
        probed = { file, ms: performance.now() - started };
        fail("projects", { name: file.name, count: projects.length }, projects);
      }
      return;
    }
    const profile = ctx.capabilities();
    if (profile !== undefined) loaded.conform(JSON.stringify(profile));
    const format = loaded.format();
    const unsupported = loaded.unsupported() as string[];
    // `null` is Rust's mark for a value that is not a finite number: keep it invalid (NaN), do not default it.
    const raw = JSON.parse(loaded.timings()) as Record<"parse" | "build" | "compute" | "span", number | null>;
    const rust = { parse: raw.parse ?? Number.NaN, build: raw.build ?? Number.NaN, compute: raw.compute ?? Number.NaN, span: raw.span ?? Number.NaN };
    // Asked again now: the read took time, and a drag, a cell edit or an edit may have begun since the start.
    const refusal = ctx.guard.refusal(turn);
    if (refusal !== null) {
      loaded.free(); // `App.open` takes the schedule; a swap that does not happen must release it.
      return fail("not-opened", { name: file.name, why: refusal });
    }
    try {
      app.open(loaded);
    } catch (err) {
      fail("unopenable", { name: file.name, why: ctx.reason(err) });
      return;
    }
    const frames = ctx.frames();
    const swapped = performance.now();
    const opened: OpenedFile = { name: file.name, bytes: file.size, format, tasks: app.task_count() };
    if (probeMs !== undefined) opened.probeMs = probeMs;
    ctx.opened(opened, unsupported, performance.now() - started, expectedNotes);
    root.focus({ preventScroll: true });
    // Every window below starts where the one before it ended, so no time is left out and none is counted twice.
    // `copy` is what the call took beyond its Rust phases, so it absorbs an under-reported phase and the sum still
    // balances. The check is `rustSpan`: Rust's first to last clock read, which `measure.mjs` compares with the sum of
    // the three phases, which are cut from those same reads. `copy` is not clamped: a negative one (Rust claims more
    // time than the call took) is a wrong measurement and is kept as one.
    const inRust = rust.parse + rust.build + rust.compute;
    unframed = {
      file: opened,
      frames,
      swapped,
      started,
      phases: {
        read: read - started,
        paint: called - read,
        copy: returned - called - inRust,
        parse: rust.parse,
        build: rust.build,
        compute: rust.compute,
        swap: swapped - returned,
        rustSpan: rust.span,
      },
    };
  };

  /** The first frame after the swap: finish the timings of the last open. */
  const frame = (frames: number): void => {
    const pending = unframed;
    if (pending === null || frames <= pending.frames) return;
    unframed = null;
    const at = performance.now();
    pending.file.timings = { ...pending.phases, firstFrame: at - pending.swapped, total: at - pending.started };
    ctx.timed(pending.file);
  };

  return { frame, openFile: (file, options) => open(file, options) };
}
