import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { backend, chromePath } from "./support/knobs";

// Unmount (gs#1081): `Gantt.destroy()` stops the frame loop, removes the listeners the chart put on `window`, `document` and the
// media queries, disconnects its observers, frees the WASM `App` (and with it the GPU device) and empties the root. The spec plays
// a host page: it serves a page of its own from a Vite dev server, imports `mount` from the module as an application would, and keeps
// the `Gantt` it returns. An init script counts what the page registers and captures the GPU device and the WASM memory.

/** What the init script keeps on `window` for the spec to read. */
interface Probe {
  counts(): { listeners: number; observers: number };
  device: GPUDevice | null;
  lost: { done: boolean };
  memory: WebAssembly.Memory | null;
  frames(): number;
  mount(options: { demo: number }): Promise<Chart>;
}

/** The slice of `Gantt` the spec drives. */
interface Chart {
  destroy(): void;
  backend(): string;
  open(file: File): Promise<void>;
  save(): Promise<void>;
  load(source: { document: unknown }): Promise<unknown>;
  newSchedule(): Promise<unknown>;
  exportCsv(): void;
  exportXlsx(): Promise<void>;
  exportImage(): Promise<void>;
  toDocument(): unknown;
  exportXer(): unknown;
  exportMspdi(): unknown;
  checks(): unknown;
}

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0; height: 100%">
<div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  window.__probe.mount = (options) => mount(document.getElementById("app"), { schedule: options, forceWebgl: window.__probe.forceWebgl });
  window.__probe.ready = true;
</script></body></html>`;

/** Runs before the page's own scripts: count the page's registrations and capture the device and the memory. */
function installProbe(forceWebgl: boolean): void {
  const entries: { target: EventTarget; type: string; listener: unknown; capture: boolean }[] = [];
  const watched = (target: unknown): target is EventTarget =>
    target === window || target === document || target instanceof MediaQueryList;
  const capture = (options: unknown): boolean => (typeof options === "boolean" ? options : !!(options as { capture?: boolean } | undefined)?.capture);
  const add = EventTarget.prototype.addEventListener;
  const remove = EventTarget.prototype.removeEventListener;
  EventTarget.prototype.addEventListener = function (this: EventTarget, type: string, listener: unknown, options?: unknown): void {
    if (watched(this) && listener !== null) {
      const same = entries.some((e) => e.target === this && e.type === type && e.listener === listener && e.capture === capture(options));
      if (!same) entries.push({ target: this, type, listener, capture: capture(options) });
    }
    (add as (...args: unknown[]) => void).call(this, type, listener, options);
  } as typeof add;
  EventTarget.prototype.removeEventListener = function (this: EventTarget, type: string, listener: unknown, options?: unknown): void {
    const at = entries.findIndex((e) => e.target === this && e.type === type && e.listener === listener && e.capture === capture(options));
    if (at >= 0) entries.splice(at, 1);
    (remove as (...args: unknown[]) => void).call(this, type, listener, options);
  } as typeof remove;

  const live = new Set<object>();
  for (const name of ["ResizeObserver", "MutationObserver", "IntersectionObserver"] as const) {
    const Original = (window as unknown as Record<string, new (...args: unknown[]) => { observe(...args: unknown[]): void; disconnect(): void }>)[name]!;
    class Counted extends Original {
      override observe(...args: unknown[]): void {
        live.add(this);
        super.observe(...args);
      }
      override disconnect(): void {
        live.delete(this);
        super.disconnect();
      }
    }
    (window as unknown as Record<string, unknown>)[name] = Counted;
  }

  // Frames: requestAnimationFrame callbacks that ran, so a loop that went on shows as a count that goes on rising.
  let ran = 0;
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (callback: FrameRequestCallback): number =>
    raf((now) => {
      ran += 1;
      callback(now);
    });

  const probe: Partial<Probe> & { forceWebgl: boolean; ready?: boolean } = {
    forceWebgl,
    device: null,
    lost: { done: false },
    memory: null,
    counts: () => ({ listeners: entries.length, observers: live.size }),
    frames: () => ran,
  };
  (window as unknown as { __probe: unknown }).__probe = probe;

  if (navigator.gpu) {
    const request = GPUAdapter.prototype.requestDevice;
    GPUAdapter.prototype.requestDevice = async function (this: GPUAdapter, ...args: Parameters<typeof request>): Promise<GPUDevice> {
      const device = await request.apply(this, args);
      probe.device = device;
      void device.lost.then(() => {
        probe.lost!.done = true;
      });
      return device;
    };
  }
  const keep = (instance: WebAssembly.Instance): void => {
    const memory = instance.exports["memory"];
    if (memory instanceof WebAssembly.Memory) probe.memory = memory;
  };
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = (async (...args: unknown[]) => {
    const result = await (instantiate as (...a: unknown[]) => Promise<WebAssembly.Instance | WebAssembly.WebAssemblyInstantiatedSource>)(...args);
    keep("instance" in result ? result.instance : result);
    return result;
  }) as typeof WebAssembly.instantiate;
  const streaming = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...args: Parameters<typeof streaming>) => {
    const result = await streaming(...args);
    keep(result.instance);
    return result;
  };
}

let server: ViteDevServer;
let base = "";

test.beforeAll(async () => {
  server = await createServer({
    root: fileURLToPath(new URL("..", import.meta.url)),
    logLevel: "silent",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
  });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});

test.afterAll(async () => {
  await server.close();
});

test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

test("destroy frees the loop, the listeners, the observers, the GPU device and the app", { tag: "@smoke" }, async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(installProbe, backend !== "webgpu");
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { __probe: { ready?: boolean } }).__probe.ready === true, undefined, { timeout: 120_000 });

  // The GPU device is checked on a WebGPU run, which needs a GPU machine (BACKEND=webgpu): where WebGPU fails the chart reloads itself on
  // WebGL2 (the fallback), which would end the page this test holds. Every other run is WebGL2 from the start, and says so.
  const before = await page.evaluate(() => (window as unknown as { __probe: Probe }).__probe.counts());

  const mounted = await page.evaluate(async () => {
    const probe = (window as unknown as { __probe: Probe }).__probe;
    const gantt = await probe.mount({ demo: 2000 });
    (window as unknown as { __gantt: typeof gantt }).__gantt = gantt;
    // Let the loop draw, then keep the app the test hook published: after `destroy` it must be freed.
    const start = probe.frames();
    while (probe.frames() < start + 10) await new Promise((done) => requestAnimationFrame(done));
    const hook = (window as unknown as { gantt: { app: unknown } }).gantt;
    (window as unknown as { __old: unknown }).__old = hook.app;
    return { counts: probe.counts(), backend: gantt.backend(), children: document.getElementById("app")!.childElementCount };
  });
  expect(mounted.children, "the chart is in the root").toBeGreaterThan(0);
  expect(mounted.counts.listeners, "the chart registered listeners on window or document").toBeGreaterThan(before.listeners);
  expect(mounted.counts.observers, "the chart registered observers").toBeGreaterThan(before.observers);

  const after = await page.evaluate(async () => {
    const probe = (window as unknown as { __probe: Probe }).__probe;
    const gantt = (window as unknown as { __gantt: { destroy(): void } }).__gantt;
    gantt.destroy();
    // Frames stop: nothing the chart scheduled runs once the frame that was in flight is over.
    await new Promise((done) => requestAnimationFrame(done));
    const frames = probe.frames();
    await new Promise((done) => setTimeout(done, 500));
    const old = (window as unknown as { __old: { frame(): void } }).__old;
    let threw = false;
    try {
      old.frame();
    } catch {
      threw = true;
    }
    return {
      counts: probe.counts(),
      frames: probe.frames() - frames,
      html: document.getElementById("app")!.innerHTML,
      threw,
      hasDevice: probe.device !== null,
    };
  });
  expect(after.html, "the root is empty").toBe("");
  expect(after.frames, "no frame runs after destroy").toBe(0);
  expect(after.counts, "listeners and observers are back to the pre-mount counts").toEqual(before);
  expect(after.threw, "a call on the old App throws").toBe(true);
  if (mounted.backend === "WebGPU") {
    expect(after.hasDevice, "the GPU device was captured").toBe(true);
    await page.waitForFunction(() => (window as unknown as { __probe: Probe }).__probe.lost.done, undefined, { timeout: 10_000 });
  } else {
    test.info().annotations.push({ type: "note", description: `WebGL2 run: no GPU device to lose (BACKEND=webgpu on a GPU machine checks it)` });
  }
  expect(errors, "no page error").toEqual([]);
});

test("a second destroy changes nothing, and every other method refuses once the chart is destroyed", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(installProbe, true);
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { __probe: { ready?: boolean } }).__probe.ready === true, undefined, { timeout: 120_000 });

  const result = await page.evaluate(async () => {
    const probe = (window as unknown as { __probe: Probe }).__probe;
    const gantt = await probe.mount({ demo: 200 });
    const start = probe.frames();
    while (probe.frames() < start + 5) await new Promise((done) => requestAnimationFrame(done));
    gantt.destroy();
    await new Promise((done) => requestAnimationFrame(done));
    const root = document.getElementById("app")!;
    const state = () => ({ counts: probe.counts(), html: root.innerHTML, frames: probe.frames() });
    const first = state();

    let secondThrew = false;
    try {
      gantt.destroy();
    } catch {
      secondThrew = true;
    }
    await new Promise((done) => setTimeout(done, 300));
    const second = state();

    const message = (err: unknown): string => (err instanceof Error ? err.message : String(err));
    const rejects: Record<string, string> = {};
    const file = new File([new Uint8Array(0)], "empty.gantt");
    const asyncCalls: Record<string, () => Promise<unknown>> = {
      open: () => gantt.open(file),
      save: () => gantt.save(),
      load: () => gantt.load({ document: {} }),
      newSchedule: () => gantt.newSchedule(),
      exportImage: () => gantt.exportImage(),
      exportXlsx: () => gantt.exportXlsx(),
    };
    for (const [method, call] of Object.entries(asyncCalls)) {
      try {
        await call();
        rejects[method] = "resolved";
      } catch (err) {
        rejects[method] = message(err);
      }
    }
    const throws: Record<string, string> = {};
    const syncCalls: Record<string, () => unknown> = {
      exportCsv: () => gantt.exportCsv(),
      toDocument: () => gantt.toDocument(),
      exportXer: () => gantt.exportXer(),
      exportMspdi: () => gantt.exportMspdi(),
      checks: () => gantt.checks(),
      backend: () => gantt.backend(),
    };
    for (const [method, call] of Object.entries(syncCalls)) {
      try {
        call();
        throws[method] = "returned";
      } catch (err) {
        throws[method] = message(err);
      }
    }
    return { first, second, secondThrew, rejects, throws };
  });
  expect(result.secondThrew, "a second destroy returns without throwing").toBe(false);
  expect(result.second, "a second destroy changes nothing: counts, root and frames are as the first left them").toEqual(result.first);
  expect(result.rejects, "open, save, load, newSchedule, exportImage and exportXlsx reject once destroyed").toEqual({
    open: "this chart was destroyed",
    save: "this chart was destroyed",
    load: "this chart was destroyed",
    newSchedule: "this chart was destroyed",
    exportImage: "this chart was destroyed",
    exportXlsx: "this chart was destroyed",
  });
  expect(result.throws, "the other methods throw once destroyed").toEqual({
    exportCsv: "this chart was destroyed",
    toDocument: "this chart was destroyed",
    exportXer: "this chart was destroyed",
    exportMspdi: "this chart was destroyed",
    checks: "this chart was destroyed",
    backend: "this chart was destroyed",
  });
  expect(errors, "no page error").toEqual([]);
});

test("20 mount and destroy cycles grow the WASM memory no more than the first", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(installProbe, true);
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { __probe: { ready?: boolean } }).__probe.ready === true, undefined, { timeout: 120_000 });

  const sizes = await page.evaluate(async () => {
    const probe = (window as unknown as { __probe: Probe }).__probe;
    const out: number[] = [];
    for (let cycle = 0; cycle < 21; cycle += 1) {
      const gantt = await probe.mount({ demo: 2000 });
      const start = probe.frames();
      while (probe.frames() < start + 5) await new Promise((done) => requestAnimationFrame(done));
      gantt.destroy();
      await new Promise((done) => requestAnimationFrame(done));
      out.push(probe.memory!.buffer.byteLength);
    }
    return out;
  });
  // The first mount takes the module's start-up memory and the first chart's; the 20 cycles after it must add no more than that.
  const grewFirst = sizes[0]!;
  const grewLater = sizes[20]! - sizes[0]!;
  expect(grewLater, `memory after 20 further cycles grew ${grewLater} bytes; the first mount took ${grewFirst}`).toBeLessThanOrEqual(grewFirst);
  expect(errors, "no page error").toEqual([]);
});
