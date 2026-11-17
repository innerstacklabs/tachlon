import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { chromePath } from "./support/knobs";
import { expectDrawing, onSwiftShader } from "./support/draws";

// A WebGPU failure in a host page (gs#1217): the chart switches to WebGL2 inside its own root, keeps the schedule and its unsaved
// edits, and reports it once through `on("error")`. The host page is never navigated, and the host's own listeners on its element stay.
// The spec plays a host: it serves a page of its own from a Vite dev server, imports `mount` from the module as an application would,
// and forces a WebGPU frame failure with the test-only `failNextFrame`.

interface ErrorDetail {
  source: string;
  message: string;
  fatal: boolean;
}

/** The slice of `Gantt` the spec drives. */
interface Chart {
  backend(): string;
  toDocument(): { tasks: { name: string }[]; baseline: unknown };
  on(name: "error", handler: (detail: ErrorDetail) => void): () => void;
  on(name: "change", handler: (detail: { backend: string }) => void): () => void;
  on(name: "find" | "toggle-status", handler: () => void): () => void;
  state(): { backend: string; rowCount: number };
  busy(): boolean;
  commands: { setFilter(filter: "critical"): boolean | undefined; setBaseline(): unknown; insertRow(): unknown; whatIf(tasks: number[]): unknown; whatIfLonger(task: number, days: number): unknown; zoomIn(): unknown };
  destroy(): void;
}

interface Host {
  ready?: boolean;
  mount(options?: { forceWebgl?: boolean }): Promise<Chart>;
  /** Mount a chart in a new root of the host's own, after `#app`. */
  mountNew(id: string, options?: { forceWebgl?: boolean }): Promise<Chart>;
  /** How often the chart's find and status keys reached the host. */
  heard?: { find: number; status: number };
  chart?: Chart;
  errors: ErrorDetail[];
  marker?: number;
  /** Key presses the host's own listener on its element heard. */
  keys: number;
  /** The backend each `change` event carried, in order. */
  changes?: string[];
}

interface Hook {
  app: { rename_task(task: number, name: string): void; set_baseline(): number; status(): string };
  failNextFrame(panic: boolean): void;
  status(): { frames: number; backend: string };
}

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0; height: 100%">
<div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  import { installChrome } from "/@id/@gantt/chrome";
  window.__host = {
    errors: [],
    keys: 0,
    // The file line, the unsaved mark and the import notes are the app chrome (apps/chrome): a host that shows them installs it.
    mount: async (options) => {
      const root = document.getElementById("app");
      const chart = await mount(root, { schedule: { demo: 2000 }, ...options });
      installChrome(chart, root);
      (window.__charts ??= {}).app = chart;
      return chart;
    },
    // Each chart by its root id, for a spec to drive.
    mountNew: async (id, options) => {
      const root = Object.assign(document.createElement("div"), { id, className: "gantt", tabIndex: 0 });
      root.style.cssText = "width: 1000px; height: 300px";
      document.body.append(root);
      return ((window.__charts ??= {})[id] = await mount(root, { schedule: { demo: 200 }, ...options }));
    },
  };
  window.__host.ready = true;
</script></body></html>`;

const RENAMED = "Renamed before the switch";

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

async function openHost(page: Page, errors: string[]): Promise<void> {
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { __host?: Host }).__host?.ready === true, undefined, { timeout: 120_000 });
}

test("a WebGPU frame failure switches the chart to WebGL2 in the host page, with the same tasks, the unsaved edit and one error event", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");

  let navigated = 0;
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) navigated += 1;
  });

  // Mount on the automatic backend, listen for errors at once, edit a task (the schedule is now unsaved), add the host's own key listener
  // on its element, then make the next WebGPU frame fail. All of it runs before the first frame, so the failure is the one armed here.
  const before = await page.evaluate(async (renamed) => {
    const host = (window as unknown as { __host: Host }).__host;
    host.marker = 1217;
    const chart = await host.mount();
    host.chart = chart;
    chart.on("error", (detail) => host.errors.push(detail));
    host.changes = [];
    chart.on("change", (detail) => host.changes!.push(detail.backend));
    const heard = (host.heard = { find: 0, status: 0 });
    chart.on("find", () => void (heard.find += 1));
    chart.on("toggle-status", () => void (heard.status += 1));
    document.getElementById("app")!.addEventListener("keydown", () => {
      host.keys += 1;
    });
    const backend = chart.backend();
    const hook = (window as unknown as { gantt: Hook }).gantt;
    hook.app.rename_task(1, renamed);
    const tasks = chart.toDocument().tasks.length;
    if (backend === "WebGPU") hook.failNextFrame(false);
    return { backend, tasks };
  }, RENAMED);
  expect(before.backend, "the chart mounted on WebGPU, so there is a WebGPU failure to switch from").toBe("WebGPU");

  // The switch ends with the chart on WebGL2 and drawing; a navigation ends the wait too, so a reload shows as a failure here. While the
  // switch runs, `backend()` throws and the hook is down: the predicate reads either as not yet.
  await Promise.race([
    page.waitForEvent("framenavigated", { timeout: 60_000 }).catch(() => undefined),
    page.waitForFunction(
      () => {
        try {
          const host = (window as unknown as { __host?: Host }).__host;
          const hook = (window as unknown as { gantt?: Hook }).gantt;
          return host?.chart?.backend() === "WebGL2" && hook?.status().backend === "WebGL2" && hook.status().frames >= 5;
        } catch {
          return false;
        }
      },
      undefined,
      { timeout: 60_000 },
    ),
  ]);
  expect(navigated, "the host page was not navigated").toBe(0);

  const first = await page.evaluate(() => (window as unknown as { gantt: Hook }).gantt.status().frames);
  await page.waitForTimeout(1000);
  // The host's commands, `state()` and `change` reach the new app, not the WebGPU one that was taken down: the critical filter
  // narrows the new app's rows and fires `change` with WebGL2, and no `change` after the switch carried the old app's state.
  const hosted = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = host.chart!;
    const rowsBefore = chart.state().rowCount;
    const changesBefore = host.changes!.length;
    const set = chart.commands.setFilter("critical");
    const frame = (): Promise<unknown> => new Promise((done) => requestAnimationFrame(done));
    for (let k = 0; k < 10; k++) await frame();
    return {
      set,
      state: chart.state().backend,
      narrowed: chart.state().rowCount < rowsBefore,
      changes: host.changes!,
      newChanges: host.changes!.slice(changesBefore),
    };
  });
  expect(hosted.set, "the host command was taken").toBe(true);
  expect(hosted.state, "state() reads the new app").toBe("WebGL2");
  expect(hosted.narrowed, "the host command drove the new app").toBe(true);
  expect(hosted.newChanges, "the filter fires change once, from the new app").toEqual(["WebGL2"]);
  expect(hosted.changes.filter((backend) => backend === "WebGPU"), `no change carried the old app's state (${JSON.stringify(hosted.changes)})`).toEqual([]);

  // A key on the chart reaches the host's own listener, and the chart's own key handling runs once, on the new app.
  await page.focus("#app");
  await page.keyboard.press("ArrowDown");
  const after = await page.evaluate(() => {
    const host = (window as unknown as { __host: Host }).__host;
    return {
      marker: host.marker,
      backend: host.chart!.backend(),
      names: host.chart!.toDocument().tasks.map((task) => task.name),
      drawn: (window as unknown as { gantt: Hook }).gantt.status().frames,
      errors: host.errors,
      canvases: document.getElementById("app")!.querySelectorAll("canvas").length,
      unsavedMark: !(document.getElementById("unsaved-mark")?.hidden ?? true),
      keys: host.keys,
    };
  });
  expect(navigated, "the host page was not navigated").toBe(0);
  expect(after.marker, "the host page's own state is still there").toBe(1217);
  expect(after.backend, "the chart draws with WebGL2").toBe("WebGL2");
  expect(after.drawn, "the chart is drawing: frames went on after the switch").toBeGreaterThan(first);
  expect(after.names.length, "the schedule is the one that was loaded").toBe(before.tasks);
  expect(after.names, "the edit made before the switch is kept").toContain(RENAMED);
  expect(after.unsavedMark, "the edit is still unsaved: the mark beside the file name is on").toBe(true);
  expect(after.canvases, "one canvas in the root").toBe(1);
  expect(after.errors.length, `one error event (${JSON.stringify(after.errors)})`).toBe(1);
  expect(after.errors[0], "the error names WebGL2 and the reason, and the chart did not stop").toMatchObject({ source: "backend", fatal: false });
  expect(after.errors[0]!.message).toMatch(/WebGL2/);
  expect(after.keys, "the host's own listener on its element still hears keys").toBe(1);
  expect(pageErrors, "no page error").toEqual([]);

  // The status key and the find key fire their event once each, through the handlers the host added before the switch. The chart
  // draws no status line or find box of its own now: the developer page's status line hears `toggle-status` through `on`, as this
  // host does. The find key goes last, as a host's find box would take focus.
  await page.focus("#app");
  // The ArrowDown above put the cell cursor in the grid, where `s` would edit a cell: Escape puts it away.
  await page.keyboard.press("Escape");
  await page.keyboard.press("s");
  await page.keyboard.press("Control+f");
  const heard = await page.evaluate(() => (window as unknown as { __host: Host }).__host.heard);
  expect(heard, "find and toggle-status fired once each after the switch").toEqual({ find: 1, status: 1 });

  // Taking the chart down leaves the host's own listener on its element.
  const keys = await page.evaluate(() => (window as unknown as { __host: Host }).__host.keys);
  await page.evaluate(() => (window as unknown as { __host: Host }).__host.chart!.destroy());
  await page.focus("#app");
  await page.keyboard.press("ArrowDown");
  expect(await page.evaluate(() => (window as unknown as { __host: Host }).__host.keys), "the host's listener outlives destroy").toBe(keys + 1);
  expect(pageErrors, "no page error").toEqual([]);
});

test("an app that arrives after its start timed out is freed", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  // A chart first, so the module is initialised as it is when `mount` calls `createApp`; then `createApp` with an App.create that answers late.
  const result = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    host.chart = await host.mount({ forceWebgl: true });
    // A path the browser asks the dev server for, held in a variable so the spec's own compiler does not resolve it.
    const bootPath = "/src/boot.ts";
    const boot = (await import(/* @vite-ignore */ bootPath)) as {
      createApp(canvas: HTMLCanvasElement, box: DOMRect, forceWebgl: boolean, schedule: unknown, create: unknown, timeoutMs: number): Promise<unknown>;
    };
    let freed = 0;
    const late = (): Promise<{ free(): void }> =>
      new Promise((done) => setTimeout(() => done({ free: () => void (freed += 1) }), 400));
    const canvas = document.createElement("canvas");
    let rejected = "";
    try {
      await boot.createApp(canvas, canvas.getBoundingClientRect(), false, null, late, 100);
    } catch (err) {
      rejected = err instanceof Error ? err.message : String(err);
    }
    await new Promise((done) => setTimeout(done, 700));
    host.chart.destroy();
    return { rejected, freed };
  });
  expect(result.rejected, "the start gave up at its deadline").toMatch(/timed out/);
  expect(result.freed, "the app that came late was freed").toBe(1);
  expect(pageErrors, "no page error").toEqual([]);
});

/** Wait until the chart draws on WebGL2 after the switch; while it runs, `backend()` throws and the hook is down. */
async function switched(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      try {
        const host = (window as unknown as { __host?: Host }).__host;
        const hook = (window as unknown as { gantt?: Hook }).gantt;
        return host?.chart?.backend() === "WebGL2" && hook?.status().backend === "WebGL2" && hook.status().frames >= 5;
      } catch {
        return false;
      }
    },
    undefined,
    { timeout: 60_000 },
  );
}

test("a baseline set before the switch is kept: setting one moves no revision, but it is an unsaved change", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  const before = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = await host.mount();
    host.chart = chart;
    const hook = (window as unknown as { gantt: Hook }).gantt;
    const rows = hook.app.set_baseline();
    const backend = chart.backend();
    if (backend === "WebGPU") hook.failNextFrame(false);
    return { backend, rows, baseline: chart.toDocument().baseline !== null };
  });
  expect(before.backend, "the chart mounted on WebGPU").toBe("WebGPU");
  expect(before.rows, "the baseline holds rows").toBeGreaterThan(0);
  expect(before.baseline, "the document holds the baseline before the switch").toBe(true);
  await switched(page);
  const after = await page.evaluate(() => {
    const host = (window as unknown as { __host: Host }).__host;
    return {
      baseline: host.chart!.toDocument().baseline !== null,
      unsavedMark: !(document.getElementById("unsaved-mark")?.hidden ?? true),
    };
  });
  expect(after.baseline, "the baseline set before the switch is still there").toBe(true);
  expect(after.unsavedMark, "and it is still unsaved").toBe(true);
  expect(pageErrors, "no page error").toEqual([]);
});

test("a start that throws part-way takes down what it put up and frees its app", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  const result = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const root = document.getElementById("app")!;
    // Count what is registered on the window, the document and the root, and taken off again.
    const live = new Set<string>();
    const ids = new WeakMap<object, number>();
    let next = 0;
    const id = (o: object): number => {
      if (!ids.has(o)) ids.set(o, next++);
      return ids.get(o)!;
    };
    const key = (target: EventTarget, type: string, listener: unknown, options: unknown): string =>
      `${id(target)} ${type} ${listener === null ? "-" : id(listener as object)} ${typeof options === "boolean" ? options : !!(options as { capture?: boolean } | undefined)?.capture}`;
    for (const target of [window, document, root] as EventTarget[]) {
      const add = target.addEventListener.bind(target);
      const remove = target.removeEventListener.bind(target);
      (target as unknown as Record<string, unknown>).addEventListener = (type: string, listener: EventListener, options?: unknown) => {
        live.add(key(target, type, listener, options));
        add(type, listener, options as AddEventListenerOptions);
      };
      (target as unknown as Record<string, unknown>).removeEventListener = (type: string, listener: EventListener, options?: unknown) => {
        live.delete(key(target, type, listener, options));
        remove(type, listener, options as EventListenerOptions);
      };
    }
    // The last step of a start puts focus in the chart: make it throw, after every install has run.
    root.focus = () => {
      throw new Error("deliberate failure at the end of the start");
    };
    let rejected = "";
    try {
      await host.mount({ forceWebgl: true });
    } catch (err) {
      rejected = err instanceof Error ? err.message : String(err);
    }
    let appFreed = false;
    try {
      (window as unknown as { gantt: Hook }).gantt.app.status();
    } catch {
      appFreed = true;
    }
    return { rejected, left: [...live], appFreed };
  });
  expect(result.rejected, "mount rejected with the failure").toMatch(/deliberate failure/);
  expect(result.left, "nothing the start registered is left on the window, the document or the root").toEqual([]);
  expect(result.appFreed, "the app the start made was freed").toBe(true);
  expect(pageErrors, "no page error").toEqual([]);
});

test("a switch after the chart has drawn fires one change, from the new app, and no stale one, and no other event", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  // Wait for the WebGPU chart's first drawn frame (`load`), then fail a frame: the events have a last status and a last view state
  // from the old app when the switch comes. (A WebGPU device the browser loses by itself after its first frame does the same.)
  const before = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = await host.mount();
    host.chart = chart;
    host.changes = [];
    let loaded = false;
    chart.on("change", (detail) => host.changes!.push(detail.backend));
    // The counters' events: none may fire from comparing the new app's first status with the old app's last one.
    const others: string[] = [];
    (window as unknown as { __others: string[] }).__others = others;
    for (const name of ["edit", "open", "select", "move"]) {
      (chart as unknown as { on(name: string, handler: () => void): void }).on(name, () => others.push(name));
    }
    (chart as unknown as { on(name: "load", handler: () => void): void }).on("load", () => {
      loaded = true;
    });
    const backend = chart.backend();
    while (!loaded) await new Promise((done) => requestAnimationFrame(done));
    // An edit, so the old app's revision is not the new one's (a kept document starts at revision 0).
    const hook = (window as unknown as { gantt: Hook }).gantt;
    hook.app.rename_task(1, "Renamed after the first frame");
    await new Promise((done) => requestAnimationFrame(done));
    await new Promise((done) => requestAnimationFrame(done));
    others.length = 0;
    try {
      if (chart.backend() === "WebGPU") (window as unknown as { gantt: Hook }).gantt.failNextFrame(false);
    } catch {
      // Already switching: the browser lost the device after the first frame.
    }
    return { backend };
  });
  expect(before.backend, "the chart mounted on WebGPU").toBe("WebGPU");
  await switched(page);
  await page.waitForTimeout(500);
  const changes = await page.evaluate(() => (window as unknown as { __host: Host }).__host.changes!);
  const others = await page.evaluate(() => (window as unknown as { __others: string[] }).__others);
  expect(changes.at(-1), "the last change carries the new app's state").toBe("WebGL2");
  expect(changes.filter((backend) => backend === "WebGPU").length, "a change from the old app's own frames is allowed only before the switch").toBeLessThanOrEqual(1);
  expect(changes.filter((backend) => backend === "WebGL2"), "one change after the switch").toEqual(["WebGL2"]);
  expect(others, "no edit, open, select or move from comparing the new app with the old").toEqual([]);
  expect(pageErrors, "no page error").toEqual([]);
});

test("a file the host opened is still the opened file after the switch: the toolbar's file line and the test hook", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  const fixture = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url), "utf8");
  // Mount on WebGPU, open a file through the host API, then fail a frame (or let the browser lose the device by itself).
  const before = await page.evaluate(async (text) => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = await host.mount();
    host.chart = chart;
    const backend = chart.backend();
    await (chart as unknown as { open(file: File): Promise<void> }).open(new File([text], "office-fitout.xml"));
    const opened = (window as unknown as { gantt?: { opened?: { name: string; tasks: number } } }).gantt?.opened ?? null;
    // The chrome's file line follows the next `change`, a frame after the open.
    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
    const line = document.getElementById("open-file")?.textContent ?? "";
    try {
      if (chart.backend() === "WebGPU") (window as unknown as { gantt: Hook }).gantt.failNextFrame(false);
    } catch {
      // Already switching.
    }
    return { backend, opened, line };
  }, fixture);
  expect(before.backend, "the chart mounted on WebGPU").toBe("WebGPU");
  expect(before.opened?.name, "the hook records the opened file").toBe("office-fitout.xml");
  expect(before.line, "the toolbar names the opened file").toContain("office-fitout");
  await switched(page);
  const after = await page.evaluate(() => ({
    opened: (window as unknown as { gantt: { opened?: { name: string; tasks: number } } }).gantt.opened ?? null,
    line: document.getElementById("open-file")?.textContent ?? "",
    fileName: ((window as unknown as { __host: Host }).__host.chart as unknown as { state(): { fileName: string | null } }).state().fileName,
  }));
  expect(after.fileName, "state() still names the opened file").toBe("office-fitout.xml");
  expect(after.line, "the new start's toolbar still names the file the schedule came from").toBe(before.line);
  expect(after.opened, "the new start's hook still names the file the schedule came from").toEqual(before.opened);
  expect(pageErrors, "no page error").toEqual([]);
});

test("while the chart switches, the edit commands refuse with busy and the others give false", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  // Fail a frame. The switch gives no timer a turn, but the host's own observer of its element runs inside it, when the old start's
  // children go: the commands are called there, with the old start down and the new one not up.
  const during = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = (host.chart = await host.mount());
    const backend = chart.backend();
    const seen = new Promise<{ busy: boolean; answers: string }>((done) => {
      const observer = new MutationObserver(() => {
        if (!chart.busy()) return;
        observer.disconnect();
        const { commands } = chart;
        done({ busy: true, answers: JSON.stringify([commands.setBaseline(), commands.insertRow(), commands.whatIf([0]), commands.whatIfLonger(0, 1), commands.setFilter("critical"), commands.zoomIn()]) });
      });
      observer.observe(document.getElementById("app")!, { childList: true });
    });
    if (backend === "WebGPU") (window as unknown as { gantt: Hook }).gantt.failNextFrame(false);
    return { backend, ...(await seen) };
  });
  expect(during.backend, "the chart mounted on WebGPU").toBe("WebGPU");
  expect(during.busy, "the switch was running").toBe(true);
  expect(during.answers, "busy for the edit commands, whatIf and whatIfLonger, false for the rest").toBe(JSON.stringify([{ ok: false, reason: "busy", message: "Finish the drag first." }, { ok: false, reason: "busy", message: "Finish the drag first." }, { ok: false, reason: "busy", message: "Finish the drag first." }, { ok: false, reason: "busy", message: "Finish the drag first." }, false, false]));
  await switched(page);
  expect(pageErrors, "no page error").toEqual([]);
});

test("a switch whose WebGL2 start fails still hands the host the document it kept, with the edit", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  const result = await page.evaluate(async (renamed) => {
    const host = (window as unknown as { __host: Host }).__host;
    const chart = (host.chart = await host.mount());
    const backend = chart.backend();
    const fatal = new Promise<ErrorDetail>((done) => chart.on("error", (detail) => detail.fatal && done(detail)));
    (window as unknown as { gantt: Hook }).gantt.app.rename_task(1, renamed);
    // The last step of a start puts focus in the chart: the WebGL2 start of the switch throws there.
    document.getElementById("app")!.focus = () => {
      throw new Error("deliberate failure of the WebGL2 start");
    };
    if (backend === "WebGPU") (window as unknown as { gantt: Hook }).gantt.failNextFrame(false);
    const error = await fatal;
    try {
      return { backend, error, names: chart.toDocument().tasks.map((task) => task.name) };
    } catch (err) {
      return { backend, error, names: [], threw: String(err) };
    }
  }, RENAMED);
  expect(result.backend, "the chart mounted on WebGPU").toBe("WebGPU");
  expect(result.error.message, "the error says WebGL2 could not start").toMatch(/WebGL2 could not start/);
  expect(result, "toDocument returns the kept document").not.toHaveProperty("threw");
  expect(result.names, "the edit made before the switch is in it").toContain(RENAMED);
  expect(pageErrors, "no page error").toEqual([]);
});

/**
 * Mount chart 1 in `#app` on WebGL2, then chart 2 in `#other` (WebGL2, or WebGPU with `gpuOther`) and
 * chart 3 in `#third` on WebGPU, whose hook is `window.gantt`; take chart 1 down if `dropFirst`, then fail chart 3's next WebGPU frame
 * (a browser that loses the device by itself has already started that switch). Resolves each chart's backend as it mounted.
 */
async function threeCharts(page: Page, gpuOther: boolean, dropFirst: boolean): Promise<string[]> {
  return page.evaluate(async ([gpuOther, dropFirst]) => {
    const host = (window as unknown as { __host: Host }).__host;
    const fatal: string[] = ((window as unknown as { __fatal: string[] }).__fatal = []);
    const first = await host.mount({ forceWebgl: true });
    const other = await host.mountNew("other", { forceWebgl: !gpuOther });
    // Chart 2's own hook (`window.gantt` is the last mounted chart's), to read its frames.
    (window as unknown as { __otherHook: unknown }).__otherHook = (window as unknown as { gantt: Hook }).gantt;
    const chart = (host.chart = await host.mountNew("third"));
    for (const c of [other, chart]) c.on("error", (detail) => void (detail.fatal && fatal.push(detail.message)));
    (window as unknown as { __other: Chart }).__other = other;
    const backends = [other.backend(), chart.backend()];
    if (dropFirst) first.destroy();
    try {
      if (chart.backend() === "WebGPU") (window as unknown as { gantt: Hook }).gantt.failNextFrame(false);
    } catch {
      // Already switching.
    }
    return backends;
  }, [gpuOther, dropFirst] as const);
}

/** The id of the canvas in the root `#id`: its part name after the chart's prefix. */
const canvasId = (page: Page, id: string): Promise<string | undefined> => page.evaluate((id) => document.querySelector(`#${id} canvas`)?.id, id);

test("a chart that switches keeps its number, and its ids, when a lower-numbered chart was destroyed first", { tag: "@smoke" }, async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  expect((await threeCharts(page, false, true))[1], "chart 3 mounted on WebGPU").toBe("WebGPU");
  await switched(page);
  expect(await canvasId(page, "third"), "the switched chart is still chart 3").toBe("chart-3-timeline");
  expect(await canvasId(page, "other"), "chart 2 is still chart 2").toBe("chart-2-timeline");
  expect(pageErrors, "no page error").toEqual([]);
});

test("two WebGPU charts on a page: one switches, and the other keeps drawing", async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  // Chart 1 goes first, so the old allocator would give the switching chart number 1.
  const mounted = await threeCharts(page, true, true);
  expect(mounted, "both charts mounted on WebGPU").toEqual(["WebGPU", "WebGPU"]);
  await switched(page);
  expect(await canvasId(page, "third"), "the switched chart is still chart 3").toBe("chart-3-timeline");
  expect(await canvasId(page, "other"), "the other chart is still chart 2").toBe("chart-2-timeline");
  await expectDrawing(page, "third");
  await expectDrawing(page, "other");
  // A devteam-1-662 / gantt#1355: the SwiftShader device is lost on its own (a CI-runner artifact), so only a real GPU can say the other chart stayed on WebGPU.
  if (!(await onSwiftShader(page))) {
    expect(await page.evaluate(() => (window as unknown as { __other: Chart }).__other.backend()), "the other chart is still on WebGPU").toBe("WebGPU");
  }
  expect(await page.evaluate(() => (window as unknown as { __fatal: string[] }).__fatal), "neither chart stopped").toEqual([]);
  expect(pageErrors, "no page error").toEqual([]);
});

test("three WebGPU charts on a page: destroying one leaves the other two drawing", async ({ page }) => {
  const pageErrors: string[] = [];
  await openHost(page, pageErrors);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  const result = await page.evaluate(async () => {
    const host = (window as unknown as { __host: Host }).__host;
    const fatal: string[] = ((window as unknown as { __fatal: string[] }).__fatal = []);
    const charts = [await host.mount(), await host.mountNew("other"), await host.mountNew("third")];
    for (const c of charts) c.on("error", (detail) => void (detail.fatal && fatal.push(detail.message)));
    const before = charts.map((c) => c.backend());
    charts[0]!.destroy();
    await new Promise((resolve) => setTimeout(resolve, 1000));
    return { before, after: [charts[1]!.backend(), charts[2]!.backend()] };
  });
  expect(result.before, "all three mounted on WebGPU").toEqual(["WebGPU", "WebGPU", "WebGPU"]);
  await expectDrawing(page, "other");
  await expectDrawing(page, "third");
  expect(await page.evaluate(() => (window as unknown as { __fatal: string[] }).__fatal), "no chart stopped").toEqual([]);
  // A devteam-1-662 / gantt#1355: the SwiftShader device is lost on its own (a CI-runner artifact); the real-device check is the Mac mini frame read.
  if (!(await onSwiftShader(page))) expect(result.after, "the two left are still on WebGPU").toEqual(["WebGPU", "WebGPU"]);
  expect(pageErrors, "no page error").toEqual([]);
});
