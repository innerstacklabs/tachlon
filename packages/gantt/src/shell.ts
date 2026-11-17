import { type PublicApp, type Status } from "./api.js";

/**
 * What every part of the shell shares: the chart's root and canvas, the app, its status and the number format.
 * @internal
 */
export interface Shell {
  root: HTMLDivElement;
  canvas: HTMLCanvasElement;
  app: PublicApp;
  status: () => Status;
  fmt: Intl.NumberFormat;
  /** What `Gantt.destroy` runs, last registered first: each `install*` that puts something on `window`, `document`, a media query or an observer adds the call that takes it off. */
  disposers: (() => void)[];
}

/**
 * Disconnect `observer` when the chart is destroyed. Returns it, so a call reads `watching(shell, new MutationObserver(f))`.
 * @internal
 */
export function watching<O extends { disconnect(): void }>({ disposers }: Pick<Shell, "disposers">, observer: O): O {
  disposers.push(() => observer.disconnect());
  return observer;
}

/**
 * Add `listener` to `target` (the window, the document or a media query) and take it off when the chart is destroyed.
 * @internal
 */
export function listening<T extends EventTarget>(
  { disposers }: Pick<Shell, "disposers">,
  target: T,
  type: string,
  listener: (event: never) => void,
  options?: AddEventListenerOptions,
): void {
  target.addEventListener(type, listener as EventListener, options);
  disposers.push(() => target.removeEventListener(type, listener as EventListener, options));
}

/**
 * Run `disposers`, last registered first, then `free` the app if there is one; a step that throws is logged and the rest still run.
 * @internal
 */
export function takeDown(disposers: Shell["disposers"], free?: () => void): void {
  for (const step of [...disposers.reverse(), ...(free ? [free] : [])]) {
    try {
      step();
    } catch (err) {
      console.error(err instanceof Error ? err.message : String(err));
    }
  }
}
