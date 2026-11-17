import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
import { countGpuRequests, gpuRequests } from "./support/gpu-count";

// One graphics connection per page (tl#1298): three charts that start on WebGPU together ask the browser for two adapters (the
// instance's probe and the chart's own) and one device. The counts do not depend on the GPU.
const ROOTS = ["c1", "c2", "c3"];
const HOST = `<!doctype html><html><head><title>host</title></head><body>${ROOTS.map((id) => `<div id="${id}" class="gantt" style="height: 200px"></div>`).join("")}
<script type="module">
  import { mount } from "/src/index.ts";
  const charts = await Promise.all(${JSON.stringify(ROOTS)}.map((id) => mount(document.getElementById(id), { schedule: { demo: 100 } })));
  window.backends = charts.map((chart) => chart.backend());
</script></body></html>`;

let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(() => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

test("three charts on one page make two adapter requests and one device request", async ({ page }) => {
  await countGpuRequests(page);
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU");
  await page.waitForFunction(() => "backends" in window, undefined, { timeout: 120_000 });
  expect(await page.evaluate(() => (window as unknown as { backends: string[] }).backends), "every chart starts on WebGPU").toEqual(["WebGPU", "WebGPU", "WebGPU"]);
  expect(await gpuRequests(page)).toEqual({ adapter: 2, device: 1 });
});
