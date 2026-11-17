import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
// gantt#1089 L4: a site that keeps its files elsewhere tells the chart where the engine is. The page sits on its own Vite server; the engine is
// not at the path beside the script (that path answers 404), it is served from `/elsewhere/engine.wasm`, and `wasmUrl` names it.
const ENGINE = fileURLToPath(new URL("../src/wasm/app_bg.wasm", import.meta.url));
const HOST = `<div id="g" class="gantt" style="width: 800px; height: 300px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.go = (wasmUrl) => mount(document.getElementById("g"), { tasks: [{ id: 1, name: "a", start: "2026-11-02", duration: 3 }], forceWebgl: true, wasmUrl })
    .then((g) => g.state().taskCount, (e) => e.stage);</script>`;
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
let server: ViteDevServer;
let base: string;
test.beforeEach(async ({ page }) => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.route("**/src/wasm/app_bg.wasm*", (route) => route.fulfill({ status: 404, contentType: "text/plain", body: "not found" }));
  await page.route("**/elsewhere/engine.wasm", (route) => route.fulfill({ path: ENGINE, contentType: "application/wasm" }));
  await page.goto(`${base}host.html`);
});
test.afterEach(async () => server.close());
const go = (page: import("@playwright/test").Page, url: "string" | "URL" | "none"): Promise<unknown> =>
  page.evaluate((how) => (window as unknown as { go(u?: string | URL): Promise<unknown> }).go(how === "none" ? undefined : how === "URL" ? new URL("/elsewhere/engine.wasm", location.href) : "/elsewhere/engine.wasm"), url);

test("a wasmUrl, as a string or a URL, names where the engine is and the chart reaches ready", async ({ page }) => {
  expect(await go(page, "string")).toBe(1);
  await page.reload();
  expect(await go(page, "URL")).toBe(1);
});
test("the same page without the option fails to start on the engine", async ({ page }) => {
  expect(await go(page, "none")).toBe("engine");
});
