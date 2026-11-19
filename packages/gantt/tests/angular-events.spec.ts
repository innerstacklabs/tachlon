import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

// gantt#1875: the `/angular` entry's `Gantt` (tests/angular-host.ts) on a fake `@angular/core` and tests/react-fake-core.ts, which fires its events at a fixed point, so none of this waits on a frame: `mount` is called outside Angular's zone
// once the view exists, the chart's events reach the outputs inside it, a new input mounts again, only an output with a subscriber is listened for, a failed start goes to the `ErrorHandler`, and a destroy calls `destroy()`.
const HOST = `<!doctype html><html><body><script type="module">import "/tests/angular-host.ts";</script></body></html>`;

let server: ViteDevServer;
test.beforeAll(async () => {
  const here = (name: string): string => fileURLToPath(new URL(name, import.meta.url));
  const alias = [{ find: /^\.\/index\.js$/, replacement: here("react-fake-core.ts") }, { find: /^@angular\/core$/, replacement: here("angular-fake-core.ts") }];
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", resolve: { alias }, server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());

test("the component mounts the chart outside the zone, emits its events inside it, mounts again for a new schedule, listens only for outputs with a subscriber, reports a failed start and destroys the chart", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  expect.soft(await page.evaluate("start()"), "one mount, though `ngOnChanges` came first; `change`, `load` and `ready` reach their outputs, each emitted inside the zone").toEqual({ change: 1, load: 1, ready: 1, errors: [], zone: [1, 3], mounts: [1], destroyed: 0, listened: ["change", "load"] });
  expect.soft(await page.evaluate("change(2)"), "a new schedule destroys the chart and mounts another, whose events reach the outputs too").toMatchObject({ change: 2, load: 2, ready: 2, mounts: [1, 2], destroyed: 1, listened: ["change", "change", "load", "load"] });
  expect.soft(await page.evaluate("listen()"), "a subscriber that comes after the mount listens for nothing yet").toMatchObject({ mounts: [1, 2], listened: ["change", "change", "load", "load"] });
  expect.soft(await page.evaluate("change(3)"), "the next mount listens for `find` too").toMatchObject({ mounts: [1, 2, 3], destroyed: 2, listened: ["change", "change", "change", "find", "load", "load", "load"] });
  expect.soft(await page.evaluate("fail()"), "a start that fails goes to the `ErrorHandler`, and its `ready` never fires").toMatchObject({ errors: ["the start failed"], ready: 3, destroyed: 3 });
  expect.soft(await page.evaluate("stop()"), "the destroy leaves no chart behind").toMatchObject({ destroyed: 3 });
  expect.soft(await page.evaluate("declaration()"), "the decorators name the class's inputs (the mount options) and its outputs (an emitter each, and `ready`), and its selector").toMatchObject({
    selector: "gantt-chart",
    standalone: true,
    inputs: { schedule: "schedule", document: "document", tasks: "tasks", links: "links", dataDate: "dataDate", mustFinishBy: "mustFinishBy", calendars: "calendars", durationUnit: "durationUnit", dateForm: "dateForm", lagCalendar: "lagCalendar", options: "options", forceWebgl: "forceWebgl", wasmUrl: "wasmUrl", locale: "locale", texts: "texts", fonts: "fonts" },
    options: ["beforeEdit", "calendars", "capabilities", "dataDate", "dateForm", "document", "durationUnit", "fields", "fonts", "forceWebgl", "lagCalendar", "links", "locale", "mustFinishBy", "options", "readOnly", "save", "schedule", "tasks", "texts", "wasmUrl"],
    emitters: ["chartChange", "chartChooseFile", "chartEdit", "chartError", "chartFind", "chartLoad", "chartMove", "chartOpen", "chartSelect", "chartToggleKeys", "chartToggleStatus", "ready"],
    outputs: Object.fromEntries(["chartChange", "chartChooseFile", "chartEdit", "chartError", "chartFind", "chartLoad", "chartMove", "chartOpen", "chartSelect", "chartToggleKeys", "chartToggleStatus", "ready"].map((n) => [n, n])),
  });
});

test("the component passes `locale`, `texts` and `fonts` to mount, and mounts again when they change", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("words(\"de\", { \"toolbar.open\": \"Offnen\" }, [1])"), "a locale and texts the page gives mount the chart again with them").toEqual([[undefined, undefined, undefined], ["de", { "toolbar.open": "Offnen" }, [1]]]);
  expect.soft(await page.evaluate("words(\"es\", { \"toolbar.open\": \"Abrir\" }, [2])"), "new ones mount it again").toEqual([[undefined, undefined, undefined], ["de", { "toolbar.open": "Offnen" }, [1]], ["es", { "toolbar.open": "Abrir" }, [2]]]);
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

test("a `save` added or removed after the mount is read when a change set is sent, and mounts nothing", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("later(true)"), "a save added after the mount is called by the chart that is mounted").toEqual({ mounts: 1, answer: "sent" });
  expect.soft(await page.evaluate("later(false)"), "removed again, there is nothing to call and nothing throws").toEqual({ mounts: 1, answer: "none" });
});

test("a new `capabilities` reaches the chart that is mounted without mounting it again, and a chart that mounts again gets the latest", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  const profile = { gestures: { barMove: false } };
  expect.soft(await page.evaluate((p) => (window as any).caps(p), profile), "the chart is given the profile, not mounted again").toEqual({ mounts: [null], profiled: [null, profile] });
  await page.evaluate("change(2)");
  expect.soft(await page.evaluate((p) => (window as any).caps(p), profile), "the next mount starts on the latest profile").toMatchObject({ mounts: [null, profile] });
});

test("a profile the chart rejects while `mount` is pending goes to the `ErrorHandler` and does not stall the next mount", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.start");
  await page.evaluate("start()");
  expect.soft(await page.evaluate("badCaps()"), "each start with the profile reports it, and a later start still mounts").toMatchObject({ errors: ["bad profile", "bad profile"], mounts: [1, 2, 3] });
});
