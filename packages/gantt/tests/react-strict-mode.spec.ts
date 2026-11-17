import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

// gantt#1096: a React app, in StrictMode, mounts the wrapper, hears `select` and unmounts it cleanly: the frame loop stops and no listener
// is left on `window` or `document`. StrictMode runs the wrapper's effect, its cleanup and the effect again, so one chart must be left.

declare global {
  interface Window {
    host: { seen: { select: number[]; load: number }; render(): void; unmount(): void };
    probe: { listeners(): number; frames(): number };
  }
}
const HOST = `<!doctype html><html><body><div id="box"></div><script type="module">import "/tests/react-host.tsx";</script></body></html>`;

/** Runs before the page's scripts: count the listeners on `window` and `document`, and the animation frames that ran. */
function installProbe(): void {
  const live: unknown[][] = [];
  const entry = (at: EventTarget, type: string, listener: unknown, o?: boolean | EventListenerOptions): unknown[] => [at, type, listener, typeof o === "boolean" ? o : !!o?.capture];
  const find = (e: unknown[]): number => live.findIndex((l) => l.every((x, i) => x === e[i]));
  for (const at of [window, document] as EventTarget[]) {
    const [add, remove] = [at.addEventListener.bind(at), at.removeEventListener.bind(at)];
    at.addEventListener = (type: string, listener: EventListenerOrEventListenerObject | null, o?: boolean | AddEventListenerOptions) => {
      if (find(entry(at, type, listener, o)) < 0) live.push(entry(at, type, listener, o));
      add(type, listener, o);
    };
    at.removeEventListener = (type: string, listener: EventListenerOrEventListenerObject | null, o?: boolean | EventListenerOptions) => {
      const i = find(entry(at, type, listener, o));
      if (i >= 0) live.splice(i, 1);
      remove(type, listener, o);
    };
  }
  let ran = 0;
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (callback) => raf((now) => void (ran += 1, callback(now)));
  window.probe = { listeners: () => live.length, frames: () => ran };
}

let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(() => server.close());

test("in StrictMode the wrapper mounts one chart, passes select on, and unmounts with no loop and no listener left", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(installProbe);
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => "host" in window, undefined, { timeout: 60_000 });
  const before = await page.evaluate(() => window.probe.listeners());

  await page.evaluate(() => window.host.render());
  await page.waitForFunction(() => window.host.seen.load === 1, undefined, { timeout: 120_000 });
  expect(await page.locator("canvas").count(), "one chart, not the two StrictMode starts").toBe(1);
  expect(await page.evaluate(() => window.probe.listeners()), "the chart is listening").toBeGreaterThan(before);

  await page.locator('#table-rows .row[data-row="1"]').click({ position: { x: 4, y: 4 } });
  await page.waitForFunction(() => window.host.seen.select.length > 0);
  expect((await page.evaluate(() => window.host.seen)).select, "select reached the callback once").toHaveLength(1);

  const after = await page.evaluate(async () => {
    window.host.unmount();
    await new Promise((done) => requestAnimationFrame(done));
    const frames = window.probe.frames();
    await new Promise((done) => setTimeout(done, 500));
    return { frames: window.probe.frames() - frames, listeners: window.probe.listeners(), canvases: document.querySelectorAll("canvas").length };
  });
  expect(after, "the loop stopped, the listeners are back, the chart is gone").toEqual({ frames: 0, listeners: before, canvases: 0 });
  expect(errors, "no page error").toEqual([]);
});
