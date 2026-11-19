import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

// gantt#1405: the wrapper hears an event the chart fires as `mount` settles, and calls `onLoad` once when another handler is added. The page runs
// the wrapper on tests/react-fake-core.ts, which fires its events at a fixed point, so neither case waits on a frame.
const HOST = `<!doctype html><html><body><script type="module">import "/tests/react-events-host.ts";</script></body></html>`;

// gantt#1494: a page that gives `wasmUrl` sees it reach `mount`, and a new `wasmUrl` mounts the chart again; gantt#1875 says the same of `locale` and `texts`. This host module exists only in the dev server.
const WASM_HOST = `import { createElement as h } from "react"; import { createRoot } from "react-dom/client"; import { Gantt } from "/src/react.tsx";
const root = createRoot(document.body.appendChild(document.createElement("div"))), schedule = { demo: 1 };
window.wasmAt = async (wasmUrl) => (root.render(h(Gantt, { schedule, wasmUrl })), await new Promise((done) => setTimeout(done, 300)), mounts.map((options) => options.wasmUrl));
window.localeAt = async (locale) => (root.render(h(Gantt, { schedule, locale })), await new Promise((done) => setTimeout(done, 300)), mounts.map((options) => options.locale));
window.textsAt = async (texts) => (root.render(h(Gantt, { schedule, texts })), await new Promise((done) => setTimeout(done, 300)), mounts.map((options) => options.texts));
window.readOnlyAt = async (readOnly) => (root.render(h(Gantt, { schedule, readOnly })), await new Promise((done) => setTimeout(done, 300)), { mounts: mounts.map((options) => options.readOnly), switched: [...(globalThis.switched ?? [])] });
window.capsAt = async (capabilities) => (root.render(h(Gantt, { schedule, capabilities })), await new Promise((done) => setTimeout(done, 300)), { mounts: mounts.map((options) => options.capabilities), profiled: [...globalThis.profiled] });
window.saveAt = async (on) => (root.render(h(Gantt, { schedule, save: on ? async () => "sent" : undefined })), await new Promise((done) => setTimeout(done, 300)), { mounts: mounts.length, answer: (await mounts[0].save[Symbol.for("gantt.liveSave")]()?.({})) ?? "none" });
window.beforeEditAt = async (answer) => (root.render(h(Gantt, { schedule, beforeEdit: () => answer })), await new Promise((done) => setTimeout(done, 300)), { mounts: mounts.length, answered: mounts[0].beforeEdit({ kind: "setDuration" }) });
window.heldReadOnly = async () => { const held = { demo: 2 }, wait = (ms) => new Promise((done) => setTimeout(done, ms)); let release; globalThis.gate = new Promise((done) => (release = done)); root.render(h(Gantt, { schedule: held, readOnly: false })); await wait(100); root.render(h(Gantt, { schedule: held, readOnly: true })); await wait(100); release(); await wait(300); return [...globalThis.switched]; };`;
const wasmHost = { name: "wasm-host", resolveId: (id: string) => (id === "/tests/wasm-host.ts" ? id : null), load: (id: string) => (id === "/tests/wasm-host.ts" ? WASM_HOST : null) };

// gantt#1522: `onReady` gets the object `mount` resolved to, once per mount (StrictMode included), and never for a mount an unmount outran.
const READY_HOST = `import { createElement as h, StrictMode } from "react"; import { createRoot } from "react-dom/client"; import { Gantt } from "/src/react.tsx";
const wait = (ms) => new Promise((done) => setTimeout(done, ms)), ready = [], schedule = { demo: 1 }, onReady = (chart) => ready.push(chart);
const start = (ui) => { const root = createRoot(document.body.appendChild(document.createElement("div"))); root.render(ui); return root; };
window.strict = async () => { const root = start(h(StrictMode, null, h(Gantt, { schedule, onReady }))); await wait(300); const seen = { mounts: mounts.length, calls: ready.length, same: ready[0] === charts[0] }; root.unmount(); await wait(100); return { ...seen, after: ready.length }; };
window.outrun = async () => { let release; globalThis.gate = new Promise((done) => (release = done)); const gone = start(h(Gantt, { schedule, onReady })); await wait(100); gone.unmount(); await wait(100); release(); await wait(300); const first = ready.length; start(h(Gantt, { schedule, onReady })); await wait(300); return { first, calls: ready.length, same: ready[0] === charts[1] }; };`;
const readyHost = { name: "ready-host", resolveId: (id: string) => (id === "/tests/ready-host.ts" ? id : null), load: (id: string) => (id === "/tests/ready-host.ts" ? READY_HOST : null) };

// gantt#1079: a start that fails is thrown while `Gantt` renders, so a host's error boundary catches it. The stand-in's `mount` rejects for the schedule `{ fail: 1 }`.
const FAIL_HOST = `import { Component, createElement as h } from "react"; import { createRoot } from "react-dom/client"; import { Gantt } from "/src/react.tsx";
class Boundary extends Component { state = { error: null }; static getDerivedStateFromError(error) { return { error }; } render() { return this.state.error ? h("p", { id: "caught" }, this.state.error.message) : this.props.children; } }
window.failed = async () => { createRoot(document.body.appendChild(document.createElement("div"))).render(h(Boundary, null, h(Gantt, { schedule: { fail: 1 } }))); await new Promise((done) => setTimeout(done, 300)); return document.getElementById("caught")?.textContent; };`;
const failHost = { name: "fail-host", resolveId: (id: string) => (id === "/tests/fail-host.ts" ? id : null), load: (id: string) => (id === "/tests/fail-host.ts" ? FAIL_HOST : null) };

let server: ViteDevServer;
test.beforeAll(async () => {
  const alias = [{ find: /^\.\/index\.js$/, replacement: fileURLToPath(new URL("react-fake-core.ts", import.meta.url)) }];
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", resolve: { alias }, plugins: [wasmHost, readyHost, failHost], server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());

test("the wrapper keeps an early `change`, and calls onLoad once when a handler is added", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.run");
  expect.soft(await page.evaluate("run(false)"), "mount settles, then the chart fires change and load").toEqual({ change: 1, load: 1 });
  expect.soft(await page.evaluate("run(true)"), "onSelect is added: no second load").toEqual({ change: 1, load: 1 });
});

test("the wrapper passes `wasmUrl` to mount, and mounts again when it changes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.wasmAt");
  expect.soft(await page.evaluate("wasmAt(\"/engine/a.wasm\")"), "the first mount gets the address the page gave").toEqual(["/engine/a.wasm"]);
  expect.soft(await page.evaluate("wasmAt(\"/engine/b.wasm\")"), "a new address mounts the chart again").toEqual(["/engine/a.wasm", "/engine/b.wasm"]);
});

test("the wrapper passes `locale` to mount, and mounts again when it changes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.localeAt");
  expect.soft(await page.evaluate("localeAt(\"de\")"), "the first mount gets the locale the page gave").toEqual(["de"]);
  expect.soft(await page.evaluate("localeAt(\"pt-BR\")"), "a new locale mounts the chart again").toEqual(["de", "pt-BR"]);
});

test("the wrapper passes `texts` to mount, and mounts again when a new object comes", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.textsAt");
  expect.soft(await page.evaluate("textsAt({ \"toolbar.open\": \"Offnen\" })"), "the first mount gets the texts the page gave").toEqual([{ "toolbar.open": "Offnen" }]);
  expect.soft(await page.evaluate("textsAt({ \"toolbar.open\": \"Abrir\" })"), "a new object mounts the chart again").toEqual([{ "toolbar.open": "Offnen" }, { "toolbar.open": "Abrir" }]);
});

test("onReady gets the chart `mount` resolved to, once per mount in StrictMode, and not after an unmount", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "ready-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.strict");
  expect.soft(await page.evaluate("strict()"), "one mount, one call with that chart, none after the unmount").toEqual({ mounts: 1, calls: 1, same: true, after: 1 });
  await page.reload();
  await page.waitForFunction("window.outrun");
  expect.soft(await page.evaluate("outrun()"), "a mount the unmount outran never calls it; the next mount calls it with its own chart").toEqual({ first: 0, calls: 1, same: true });
});

test("a chart whose start fails is thrown while `Gantt` renders, and a host's error boundary shows the error", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "fail-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.failed");
  expect(await page.evaluate("failed()"), "the boundary shows the message `mount` rejected with").toBe("the start failed");
});

test("the wrapper passes `readOnly` to mount, and a new value switches the chart that is mounted without mounting it again", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.readOnlyAt");
  expect.soft(await page.evaluate("readOnlyAt(true)"), "the first mount gets the value the page gave").toEqual({ mounts: [true], switched: [true] });
  expect.soft(await page.evaluate("readOnlyAt(false)"), "the chart is switched on its own, not mounted again").toEqual({ mounts: [true], switched: [true, false] });
  expect.soft(await page.evaluate("heldReadOnly()"), "a change while `mount` is pending is applied once it settles").toEqual([true, false, true]);
});

test("the wrapper passes `capabilities` to mount, and a new profile reaches the chart that is mounted without mounting it again", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.capsAt");
  const [first, second] = [{ gestures: { barMove: false } }, { editable: { dates: false } }];
  expect.soft(await page.evaluate((p) => (window as any).capsAt(p), first), "the first mount gets the profile the page gave").toEqual({ mounts: [first], profiled: [first] });
  expect.soft(await page.evaluate((p) => (window as any).capsAt(p), second), "a new profile is set on the chart, which is not mounted again").toEqual({ mounts: [first], profiled: [first, second] });
});

test("a `save` removed or added after the mount is read when a change set is sent, and mounts nothing", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.saveAt");
  expect.soft(await page.evaluate("saveAt(false)"), "no save at the mount: there is nothing to call and nothing throws").toEqual({ mounts: 1, answer: "none" });
  expect.soft(await page.evaluate("saveAt(true)"), "a save added later is called by the chart that is mounted").toEqual({ mounts: 1, answer: "sent" });
  expect.soft(await page.evaluate("saveAt(false)"), "removed again, there is nothing to call and nothing throws").toEqual({ mounts: 1, answer: "none" });
});

test("a new `beforeEdit` reaches the chart that is mounted and mounts nothing", async ({ page }) => {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST.replace("react-events-host", "wasm-host") }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.beforeEditAt");
  expect.soft(await page.evaluate("beforeEditAt('first')"), "the chart asks the function the page gave").toEqual({ mounts: 1, answered: "first" });
  expect.soft(await page.evaluate("beforeEditAt('second')"), "the same chart asks the new one").toEqual({ mounts: 1, answered: "second" });
});
