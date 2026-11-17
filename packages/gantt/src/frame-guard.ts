// The shell's answer to a frame that throws (gs#163). The render loop keeps running through a few
// failures, shows a banner with the error and keeps the last good frame on the canvas. After
// MAX_FAILURES in a row, or at once if the WASM module has panicked, it stops asking for frames and
// says so; it offers a reload only when the page gave one. A panic leaves the module unusable, so no retry can succeed. Each chart on a page has
// its own guard, and a failed frame counts in its own chart only; but the charts share one module, so
// a panic in one stops every chart, and each says so (gs#1083).

import { optional } from "./dom.js";
import { say } from "./messages.js";
import { last_panic } from "./wasm/app.js";

/** Failed frames in a row before the loop stops retrying. */
export const MAX_FAILURES = 3;

export interface FrameGuardOptions {
  /** The chart root, where the banner is looked up. */
  root: ParentNode;
  /** The one-line text for an error (the panic message if there is one). */
  describe(err: unknown): string;
  /** Whether the WASM module has panicked, which leaves it unusable. Default: `last_panic()` has a record. */
  panicked?(): boolean;
  /** Start the page again, as the page that holds the chart chooses. Without it the banner offers no reload: the host decides. */
  reload?(): void;
  /** Stop this chart's loop and report `why`: another chart's frame left the module unusable. */
  halt?(why: string): void;
}

export interface FrameGuard {
  /**
   * Run one frame. If it throws, `onFail` may take the error over (return true: the guard counts
   * nothing and does not retry); otherwise the guard records it, shows the banner and, while the
   * loop may go on, asks for `next` again. Returns whether the frame drew, so the caller can stop
   * when it did not. A frame that drew starts the failures in a row again from zero.
   */
  run(frame: () => void, onFail: (err: unknown) => boolean, next: FrameRequestCallback): boolean;
  /** Leave the page's set of guards, so a later panic elsewhere no longer reaches this chart. */
  dispose(): void;
  /** Whether `err`, if the guard records it, stops the loop: a panic, or the last failure it retries. */
  stops(err: unknown): boolean;
}

/** What stops each live guard's chart, for a panic in any chart of the page. */
const guards = new Set<(why: string) => void>();

/** What `WebAssembly.RuntimeError: unreachable` looks like, which is how a Rust panic reaches the shell. */
function isTrap(err: unknown): boolean {
  return err instanceof WebAssembly.RuntimeError && /unreachable/i.test(err.message);
}

/** Whether `err` came with a panic, after which the WASM module cannot be used again on this page. */
export function unusable(err: unknown): boolean {
  return last_panic() != null || isTrap(err);
}

/**
 * The banner is the part `frame-error` (with `frame-error-text`, `frame-error-dismiss` and
 * `frame-error-reload`); the chart works without it, and an error is then only logged.
 */
export function createFrameGuard(options: FrameGuardOptions): FrameGuard { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const banner = optional<HTMLElement>(options.root, "frame-error");
  const text = optional<HTMLElement>(options.root, "frame-error-text");
  const dismiss = optional<HTMLElement>(options.root, "frame-error-dismiss");
  const reload = optional<HTMLElement>(options.root, "frame-error-reload");
  let failures = 0;
  let lastWhy = "";

  dismiss?.addEventListener("click", () => {
    if (banner !== null) banner.hidden = true;
  });
  const restart = options.reload;
  if (restart !== undefined) reload?.addEventListener("click", () => restart());

  /** The banner's final text; it points at the Reload button only when there is one. */
  const stopped = (why: string): string => say(options.root, restart === undefined ? "render-stopped" : "render-stopped-reload", { why });
  const show = (message: string, final: boolean): void => {
    if (banner === null || text === null) return;
    text.textContent = message;
    if (dismiss !== null) dismiss.hidden = final;
    if (reload !== null) reload.hidden = !final || restart === undefined;
    banner.hidden = false;
  };

  const panicked = (err: unknown): boolean => (options.panicked ?? (() => last_panic() != null))() || isTrap(err);
  const stops = (err: unknown): boolean => panicked(err) || failures + 1 >= MAX_FAILURES;

  const failed = (err: unknown): boolean => {
    const stop = stops(err);
    const broken = panicked(err);
    failures += 1;
    const why = options.describe(err);
    lastWhy = why;
    console.error("gantt: frame failed", err);
    if (stop) {
      // This chart's loop stops here; after a panic, every other chart of the page is told to stop too.
      guards.delete(halt);
      if (broken) for (const other of [...guards]) other(why);
      show(stopped(why), true);
      return false;
    }
    show(say(options.root, "render-failed", { why, failed: failures, max: MAX_FAILURES }), false);
    return true;
  };

  const halt = (why: string): void => {
    guards.delete(halt);
    show(stopped(why), true);
    options.halt?.(why);
  };
  guards.add(halt);

  const succeeded = (): void => {
    if (failures === 0) return;
    failures = 0;
    show(say(options.root, "render-recovered", { why: lastWhy }), false);
  };

  return {
    stops,
    run(frame, onFail, next) {
      try {
        frame();
      } catch (err) {
        if (!onFail(err) && failed(err)) requestAnimationFrame(next);
        return false;
      }
      succeeded();
      return true;
    },
    dispose() {
      guards.delete(halt);
    },
  };
}
