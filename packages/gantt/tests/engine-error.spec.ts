import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, type Page, test } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product";
import { chromePath } from "./support/knobs";
// tl#1249 Row 1: a start that fails says which failure it is, with a stable code, a fix to act on and a docs pointer. Each case
// is a failure recorded in Chromium: the engine address answers 404 or a page, the page's CSP lacks `wasm-unsafe-eval`, the browser
// has no graphics, a `File` is gone. The host page calls `mount` when the spec says, and the spec reads what it rejected with.
const HOST = `<input id="f" type="file"><div id="g" class="gantt" style="width: 800px; height: 300px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.go = (schedule, webgl = true) => mount(document.getElementById("g"), schedule ? { schedule: document.getElementById("f").files[0] } : { tasks: [{ id: 1, name: "a", start: "2026-11-02", duration: 3 }], forceWebgl: webgl })
    .then(() => null, (e) => ({ name: e.name, stage: e.stage, code: e.code, fix: e.fix, docs: e.docs, message: e.message }));</script>`;
const code = (cause: string): string => `${PRODUCT_NAME.toUpperCase()}_${cause}`;
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
let server: ViteDevServer;
let base: string;
test.beforeEach(async ({ page }) => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST, headers: test.info().title.includes("CSP") ? { "content-security-policy": "script-src 'self' 'unsafe-inline'" } : {} }));
});
test.afterEach(async () => server.close());
const rejection = async (page: Page, webgl = true): Promise<Record<string, string>> => {
  await page.goto(`${base}host.html`);
  return (await page.evaluate(`window.go(false, ${webgl})`)) as Record<string, string>;
};
const engine = (page: Page, answer: { status: number; contentType: string; body: string }): Promise<unknown> => page.route("**/app_bg.wasm*", (route) => route.fulfill(answer));

test("an engine file that answers 404 rejects with ENGINE_FETCH and the status", async ({ page }) => {
  await engine(page, { status: 404, contentType: "text/plain", body: "not found" });
  const err = await rejection(page);
  expect(err).toMatchObject({ name: "MountError", stage: "engine", code: code("ENGINE_FETCH"), docs: "README.md#start-errors" });
  expect(err.message).toContain("status 404");
  expect(err.fix).toContain("app_bg.wasm");
});
test("an engine address that answers a web page rejects with ENGINE_NOT_WASM and the bytes it starts with", async ({ page }) => {
  await engine(page, { status: 200, contentType: "text/html", body: "<!doctype html><html></html>" });
  const err = await rejection(page);
  expect(err).toMatchObject({ stage: "engine", code: code("ENGINE_NOT_WASM") });
  expect(err.message).toContain('starts with "<!do"');
});
test("a page CSP without wasm-unsafe-eval rejects with ENGINE_CSP", async ({ page }) => {
  expect(await rejection(page)).toMatchObject({ stage: "engine", code: code("ENGINE_CSP"), fix: expect.stringContaining("wasm-unsafe-eval") });
});
test("a browser with no WebGPU adapter and no WebGL2 rejects with GPU_NONE", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "gpu", { value: { requestAdapter: async () => null } }); // Chrome's `navigator.gpu` with no adapter
    const get = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, kind: string, ...rest: unknown[]) { return kind === "webgl2" ? null : (get as Function).call(this, kind, ...rest); } as typeof get;
  });
  expect(await rejection(page, false)).toMatchObject({ stage: "render", code: code("GPU_NONE") }); // WebGPU first, then the WebGL2 retry
});
test("a File that was deleted before mount reads rejects with FILE_UNREADABLE", async ({ page }) => {
  const dir = mkdtempSync(join(tmpdir(), "engine-error-"));
  const file = join(dir, "plan.json");
  writeFileSync(file, "{}");
  await page.goto(`${base}host.html`);
  await page.setInputFiles("#f", file);
  unlinkSync(file);
  const err = (await page.evaluate("window.go(true)")) as Record<string, string>;
  rmSync(dir, { recursive: true });
  expect(err).toMatchObject({ stage: "schedule", code: code("FILE_UNREADABLE") });
  expect(err.message).toContain("plan.json");
});
test("a plain-text file given as the schedule rejects with SCHEDULE_UNREADABLE and a fix", async ({ page }) => {
  await page.goto(`${base}host.html`);
  await page.setInputFiles("#f", { name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("this is not a schedule") });
  const err = (await page.evaluate("window.go(true)")) as Record<string, string>;
  expect(err).toMatchObject({ stage: "schedule", code: code("SCHEDULE_UNREADABLE"), fix: expect.stringContaining("export it again as XER or XML") });
});
