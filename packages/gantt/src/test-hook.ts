// The test hook: `window.gantt`, which the `packages/gantt/tests` specs, `demos/**` and
// the measurement harness read the app through. This is the only file that
// names the hook or holds the full `App` type for it; `crates/app/tests/architecture/main.rs`
// fails if any other file under packages/gantt/src does (see docs/WASM_API.md, "How it is enforced").
import { type App } from "./wasm/app.js";
import type { MeasurementError, MeasurementResult } from "./measure.js";
import type { PublicApp, Status } from "./api.js";
import type { OpenedFile } from "./open-schedule.js";
import type { Gantt } from "./index.js";
import type { Events } from "./events.js";

/**
 * Test hook: the verify script reads the app through this.
 * @internal
 */
export interface GanttHook {
  app: App;
  status: () => Status;
  syncTheme(): void;
  /** Make this chart's next `app.frame()` fail on purpose: a panic (the module is then unusable) or a returned error. One shot. */
  failNextFrame(panic: boolean): void;
  theme(): Record<string, string>;
  /** This chart's `Gantt.on`, once `mount` has returned. */
  on: Events["on"];
  loadMs: number;
  ready: boolean;
  /** Unsupported features reported while importing a schedule. */
  unsupported?: string[];
  /** The file the last successful open read (a file chosen, dropped or passed to `setInputFiles`); replaced by the next. */
  opened?: OpenedFile;
  /** What `mount` returned, so a spec can call the host API. */
  api?: Gantt;
  /** Why WebGPU was abandoned for WebGL2, or null if it was not. */
  fallback: string | null;
  /** The scripted run's result (or why it could not run), once `?measure=1` has finished. */
  measurement?: MeasurementResult | MeasurementError;
}

declare global {
  interface Window {
    /** @internal */
    gantt?: GanttHook;
  }
}

/**
 * `app` is typed `PublicApp` so `main.ts` never holds the full type; the hook below widens it, since it is the same object.
 * @internal
 */
export interface TestHookInit {
  app: PublicApp;
  status: () => Status;
  syncTheme: () => void;
  loadMs: number;
  fallback: string | null;
  unsupported: string[] | undefined;
}

/** What `mount` records on its own chart's hook. A later `mount` replaces `window.gantt`, and these still write to the hook they came with. */
export interface HookRecords {
  /** The scripted run's result, which it records when it finishes. */
  measured(outcome: MeasurementResult | MeasurementError): void;
  /** A successful open: the file, and the new file's unsupported list in place of the old. */
  opened(file: OpenedFile, unsupported: string[]): void;
  /** What `mount` returned. */
  api(api: Gantt): void;
  /** Take this chart's hook off `window.gantt` while it switches to WebGL2, as a reload did; the next start publishes its own. */
  drop(): void;
}

/**
 * Publish this chart's hook on `window.gantt`, and return what records on it. With several charts on a page, `window.gantt`
 * is the last mounted chart's; a spec keeps each chart's hook as its `mount` resolves.
 * @internal
 */
export function installTestHook(init: TestHookInit): HookRecords {
  const { app, status, syncTheme, loadMs, fallback, unsupported } = init;
  const theme = (): Record<string, string> => JSON.parse(app.theme()) as Record<string, string>;
  const gantt: GanttHook = {
    app: app as App,
    status,
    syncTheme,
    failNextFrame: (panic) => (app as App).test_fail_next_frame(panic),
    theme,
    on: (name, handler) => gantt.api!.on(name, handler),
    loadMs,
    ready: true,
    fallback,
  };
  if (unsupported !== undefined) gantt.unsupported = unsupported;
  window.gantt = gantt;
  return {
    measured: (outcome) => {
      gantt.measurement = outcome;
    },
    opened: (file, lines) => {
      gantt.opened = file;
      gantt.unsupported = lines;
    },
    api: (api) => {
      gantt.api = api;
    },
    drop: () => {
      if (window.gantt === gantt) delete window.gantt;
    },
  };
}
