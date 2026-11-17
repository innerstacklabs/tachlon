// What `mount` loads before it draws (gs#534): the engine, the schedule its options name, and the error it rejects with.
// `MountError` is an export of the page entry, `index.ts`, which re-exports it from here. What it says of each cause is `errors.ts`.
import init, { last_panic, LoadedSchedule, type InitOutput } from "./wasm/app.js";
import { documentText } from "./document.js";
import { fromTasks } from "./adapters/plain.js";
import { PlainDataError } from "./adapters/validate.js";
import { codeFor, engineFailure, ERROR_DOCS, type ErrorCode, fixFor, graphicsCode } from "./errors.js";
import type { MountOptions } from "./index.js";

/** Why `mount` could not start the chart: `stage` is where it failed, and `message` is one line for a person. */
export class MountError extends Error {
  readonly stage: "engine" | "schedule" | "render";
  /** A stable name for the cause when there is one (`errors.ts` lists them); `fix` says what to do about it and `docs` where it is explained. */
  readonly code?: ErrorCode;
  readonly fix?: string;
  readonly docs?: string;

  constructor(stage: "engine" | "schedule" | "render", message: string, options?: ErrorOptions & { code?: ErrorCode }) {
    super(message, options);
    this.name = "MountError";
    this.stage = stage;
    this.code = options?.code;
    if (this.code !== undefined) [this.fix, this.docs] = [fixFor(this.code), ERROR_DOCS];
  }
}

/** The render `MountError` for a failed start. In a browser with no WebGPU adapter and no WebGL2 it is `GPU_NONE`, whatever the engine's own message says; the probes run only on this failure path. */
export async function renderError(message: string, cause: unknown): Promise<MountError> {
  const gpu = (navigator as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
  const adapter = (await gpu?.requestAdapter().catch(() => null)) ?? null;
  return new MountError("render", message, { cause, code: graphicsCode(adapter !== null, () => document.createElement("canvas").getContext("webgl2") !== null) });
}

/** The engine's file when the page names none: the one `init` fetches by default, beside this script. */
const ENGINE = new URL("./wasm/app_bg.wasm", import.meta.url);

/** Where the engine is: the page's `wasmUrl` (a string reads against the page's address), else `ENGINE`. A failed load fetches it again to find out why. */
const engineAt = (wasmUrl: string | URL | undefined): URL => (typeof wasmUrl === "string" ? new URL(wasmUrl, document.baseURI) : (wasmUrl ?? ENGINE));

/** A host page's inline script may start `WebAssembly.compileStreaming` on the engine's response and keep the promise here, so the compile does not wait for this module to load (gs#1354). */
declare global {
  interface Window {
    __engineModule?: Promise<WebAssembly.Module>;
  }
}

/** The engine's load once started, so `preloadEngine` and `mount` share one request, and the first call's `wasmUrl` decides where it is fetched from; a failed load is forgotten so the next call tries again. */
let started: Promise<InitOutput> | undefined;

/**
 * The engine's exports once loaded, starting the load or joining the one started.
 * @internal The wasm types stay out of the package's typings; a host page calls `preloadEngine`.
 */
export function engine(wasmUrl?: string | URL): Promise<InitOutput> {
  if (started === undefined) {
    const mine = loadEngine(wasmUrl);
    started = mine;
    mine.catch(() => void (started === mine && (started = undefined)));
  }
  return started;
}

/** Start loading the engine, or join the load already started: a host page calls it first thing, so the file downloads and compiles while the page reads its data (gs#1336). Settles with no value when the engine has loaded. */
export function preloadEngine(options?: Pick<MountOptions, "wasmUrl">): Promise<void> {
  const loaded = engine(options?.wasmUrl).then(() => undefined);
  loaded.catch(() => undefined); // A caller that drops the promise gets no unhandled rejection, as `engine`'s own load gives none.
  return loaded;
}

/** Load the engine, or reject with a `MountError` of stage `"engine"` that says which failure it is. */
async function loadEngine(wasmUrl: string | URL | undefined): Promise<InitOutput> {
  const at = engineAt(wasmUrl);
  try {
    // The page's early compile when it has one; if that failed or there is none, `init` fetches and compiles the file itself.
    const early = await window.__engineModule?.catch(() => undefined);
    return await init({ module_or_path: early ?? at });
  } catch (err) {
    const response = await fetch(at).catch(() => null);
    const reader = response?.body?.getReader();
    const first = await reader?.read().catch(() => undefined);
    void reader?.cancel().catch(() => undefined);
    const { code, message } = engineFailure({ status: response?.status ?? null, head: first?.value?.subarray(0, 4) ?? new Uint8Array() }, err);
    throw new MountError("engine", message, { cause: err, code });
  }
}

/** What a chart starts on: the schedule `mount` was given (`undefined` for its `tasks` and `links`), or the document of the one it held when it switched to WebGL2. */
export type StartSource = MountOptions["schedule"] | { document: unknown } | { tasks: MountOptions["tasks"]; links: MountOptions["links"]; dataDate?: MountOptions["dataDate"]; mustFinishBy?: MountOptions["mustFinishBy"]; options?: MountOptions["options"]; calendars?: MountOptions["calendars"]; durationUnit?: MountOptions["durationUnit"]; lagCalendar?: MountOptions["lagCalendar"] };

/**
 * Build or read the schedule `source` names.
 * @internal
 */
export async function load(source: NonNullable<StartSource>, groups = 0): Promise<{ schedule: LoadedSchedule; unsupported: string[] | undefined }> {
  if ("tasks" in source) {
    let document: unknown;
    try {
      document = fromTasks({ tasks: source.tasks ?? [], links: source.links, dataDate: source.dataDate, mustFinishBy: source.mustFinishBy, options: source.options, calendars: source.calendars, durationUnit: source.durationUnit, lagCalendar: source.lagCalendar }).document;
    } catch (err) {
      throw new MountError("schedule", err instanceof Error ? err.message : String(err), { cause: err, code: err instanceof PlainDataError ? codeFor("DATA_INVALID") : undefined });
    }
    return load({ document });
  }
  if ("document" in source) {
    try {
      const schedule = LoadedSchedule.parse_document(JSON.stringify(source.document));
      return { schedule, unsupported: schedule.unsupported() as string[] };
    } catch (err) {
      throw new MountError("schedule", last_panic() ?? (err instanceof Error ? err.message : String(err)), { cause: err, code: codeFor("SCHEDULE_UNREADABLE") });
    }
  }
  if (source instanceof File) {
    let bytes: Uint8Array;
    try {
      // Bytes, not text: the wasm module sees ERMHDR and chooses XER or MS Project XML; a gzip file that holds a document is read as one.
      bytes = new Uint8Array(await source.arrayBuffer());
    } catch (err) {
      throw new MountError("schedule", `The file "${source.name}" could not be read: ${err instanceof Error ? err.message : String(err)}`, { cause: err, code: codeFor("FILE_UNREADABLE") });
    }
    try {
      const text = await documentText(bytes);
      const schedule = text === null ? LoadedSchedule.parse_bytes(bytes, "all") : LoadedSchedule.parse_document(text);
      return { schedule, unsupported: schedule.unsupported() as string[] };
    } catch (err) {
      throw new MountError("schedule", err instanceof Error ? err.message : String(err), { cause: err, code: codeFor("SCHEDULE_UNREADABLE") });
    }
  }
  try {
    const schedule = "stress" in source ? (groups > 0 ? LoadedSchedule.stress_grouped(source.stress, groups) : LoadedSchedule.stress(source.stress)) : LoadedSchedule.demo(source.demo);
    return { schedule, unsupported: undefined };
  } catch (err) {
    const text = err instanceof Error ? err.message : String(err);
    throw new MountError("schedule", last_panic() ?? text, { cause: err });
  }
}
