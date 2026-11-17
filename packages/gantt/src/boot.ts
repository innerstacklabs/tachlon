import { App, last_panic, LoadedSchedule } from "./wasm/app.js";
import { type PublicApp } from "./api.js";

const DEFAULT_N = 50_000;
const MAX_N = 200_000;

export function taskCount(params: URLSearchParams): number {
  const n = Number(params.get("n") ?? DEFAULT_N);
  return Number.isInteger(n) && n > 0 ? Math.min(n, MAX_N) : DEFAULT_N;
}

/** One line, short enough for the overlay and a URL. */
export function reason(err: unknown): string {
  const text = last_panic() ?? (err instanceof Error ? err.message : String(err));
  // A panic reads "panicked at file:line:col:" then the message: keep the message.
  const lines = text.split("\n").filter((l) => l.trim() !== "");
  const line = (/^panicked at .*:$/.test(lines[0] ?? "") ? lines[1] : lines[0]) ?? text;
  return line.length > 240 ? `${line.slice(0, 240)}…` : line;
}

/**
 * The developer page's restart: reload on WebGL2, carrying the reason WebGPU
 * failed. The chart itself switches to WebGL2 in its root (`webgl-switch.ts`);
 * the page reloads only when that cannot work, after a panic, which leaves the
 * module unusable.
 */
export function fallBackToWebgl(why: string): void {
  console.warn(`gantt: WebGPU failed, falling back to WebGL2: ${why}`);
  const params = new URLSearchParams(location.search);
  params.set("backend", "webgl");
  params.set("fallback", why);
  params.delete("panic"); // the debug panic is for the first attempt only
  location.replace(`${location.pathname}?${params}`);
}

/** Longest `App.create` may take before the shell gives up on it. */
const INIT_TIMEOUT_MS = 10_000;
const PANIC_POLL_MS = 50;

/**
 * What `createApp` calls: `App.create`, or a stand-in in a spec.
 * @internal
 */
export type CreateApp = (
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
  dpr: number,
  forceWebgl: boolean,
  schedule: LoadedSchedule,
) => Promise<PublicApp>;

/**
 * `App.create`, with a watchdog that does not depend on its promise. A Rust
 * panic inside the exported async function can leave that promise pending
 * forever, so the watchdog polls the panic hook's record and a deadline, and
 * rejects with the reason if either trips. An app that arrives after that is
 * freed at once, with its GPU device: the chart has gone on without it (to
 * WebGL2, on a fresh canvas).
 * @internal
 */
export function createApp(
  canvas: HTMLCanvasElement,
  box: DOMRect,
  forceWebgl: boolean,
  schedule: LoadedSchedule,
  create: CreateApp = (...args) => App.create(...args),
  timeoutMs: number = INIT_TIMEOUT_MS,
): Promise<PublicApp> {
  return new Promise<PublicApp>((resolve, reject) => {
    const deadline = performance.now() + timeoutMs;
    let gaveUp = false;
    const timer = setInterval(() => {
      const panic = last_panic();
      if (panic !== undefined && panic !== null) {
        clearInterval(timer);
        gaveUp = true;
        reject(new Error(panic));
      } else if (performance.now() > deadline) {
        clearInterval(timer);
        gaveUp = true;
        reject(new Error(`init timed out after ${timeoutMs / 1000} s`));
      }
    }, PANIC_POLL_MS);
    create(canvas, box.width, box.height, devicePixelRatio, forceWebgl, schedule).then(
      (app) => {
        clearInterval(timer);
        if (gaveUp) {
          console.warn("gantt: an app arrived after its start was given up, and is freed");
          try {
            app.free();
          } catch (err) {
            // After a panic the module cannot free it; nothing more can be done for it on this page.
            console.warn("gantt: the late app could not be freed", err);
          }
          return;
        }
        resolve(app);
      },
      (err: unknown) => {
        clearInterval(timer);
        reject(err);
      },
    );
  });
}
