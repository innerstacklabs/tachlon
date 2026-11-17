import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

// gantt#1875: a Vue app renders the `/vue` entry's `Gantt` (tests/vue-host.ts) on tests/react-fake-core.ts, which fires its events at a fixed point, so none of this waits on a frame: `mount` is called, the chart's events are emitted, a new mount option mounts again, only an event with a listener is listened for, and an unmount calls `destroy()`.
const HOST = `<!doctype html><html><body><script type="module">import "/tests/vue-host.ts";</script></body></html>`;

let server: ViteDevServer;
test.beforeAll(async () => {
  const alias = [{ find: /^\.\/index\.js$/, replacement: fileURLToPath(new URL("react-fake-core.ts", import.meta.url)) }];
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", resolve: { alias }, server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());

test("the component mounts the chart, emits its events, mounts again for a new schedule, listens only for events with a listener, and destroys the chart when it unmounts", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  expect.soft(await page.evaluate("start()"), "one mount; the chart's `change` and `load` reach the emits; `ready` fires once").toEqual({ change: 1, load: 1, ready: 1, mounts: [1], destroyed: 0, listened: ["change", "load"] });
  expect.soft(await page.evaluate("change(2)"), "a new schedule destroys the chart and mounts another, whose events reach the emits too").toEqual({ change: 2, load: 2, ready: 2, mounts: [1, 2], destroyed: 1, listened: ["change", "change", "load", "load"] });
  expect.soft(await page.evaluate("handle([\"onFind\"])"), "`find` gets a listener on the mounted chart with no third mount, and no event without a listener is listened for").toMatchObject({ mounts: [1, 2], destroyed: 1, listened: ["change", "change", "find", "load", "load"] });
  expect.soft(await page.evaluate("stop()"), "the unmount destroys the chart that is left").toMatchObject({ mounts: [1, 2], destroyed: 2 });
});

test("the component passes `locale` to mount, and mounts again when it changes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("lang(\"de\")"), "a locale the page gives mounts the chart again with it").toMatchObject({ locales: ["-", "de"] });
  expect.soft(await page.evaluate("lang(\"pt-BR\")"), "a new locale mounts it again").toMatchObject({ locales: ["-", "de", "pt-BR"] });
});

test("the component passes `mustFinishBy` to mount, and mounts again when it changes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("finish(\"2026-03-18\")"), "a date the page gives mounts the chart again with it").toEqual([undefined, "2026-03-18"]);
});

test("the component passes `texts` to mount, and mounts again when a new object comes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("words({ \"toolbar.open\": \"Offnen\" })"), "texts the page gives mount the chart again with them").toEqual([undefined, { "toolbar.open": "Offnen" }]);
  expect.soft(await page.evaluate("words({ \"toolbar.open\": \"Abrir\" })"), "a new object mounts it again").toEqual([undefined, { "toolbar.open": "Offnen" }, { "toolbar.open": "Abrir" }]);
});

test("`@ready` draws no Vue warning, and a failed start followed by a working one is not reported again", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("problems()"), "`ready` fired and Vue warned of nothing").toEqual({ warnings: [], errors: 0 });
  await page.evaluate("fail()");
  expect.soft(await page.evaluate("problems()"), "the failed start reaches the error handler once").toEqual({ warnings: [], errors: 1 });
  expect.soft(await page.evaluate("change(2)"), "the next start mounts and emits `ready`").toMatchObject({ ready: 2 });
  expect.soft(await page.evaluate("problems()"), "its re-render does not report the old error again").toEqual({ warnings: [], errors: 1 });
});

test("a new `readOnly` switches the chart that is mounted, without mounting it again", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("flip(true)"), "the chart is switched on its own, not mounted again").toEqual({ mounts: 1, switched: [false, true] });
  expect.soft(await page.evaluate("flip(false)"), "and switched back").toEqual({ mounts: 1, switched: [false, true, false] });
  expect.soft(await page.evaluate("hold()"), "a change while `mount` is pending is applied once it settles").toEqual([false, true, false, true]);
});
