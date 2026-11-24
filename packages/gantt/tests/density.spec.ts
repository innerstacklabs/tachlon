import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";
// The `density` mount option: the view state, the root's `--gantt-row-h`, the table rows and the bars read one height, and a value outside the three names is refused before the root is touched.
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
let server: ViteDevServer;
let base: string;
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  base = (await server.listen()).resolvedUrls!.local[0]!;
});
test.afterAll(() => server.close());
const host = (density: string): string => `<div id="g" class="gantt" tabindex="0" style="width: 1200px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts";
  const root = document.getElementById("g"), before = root.innerHTML;
  window.api = await mount(root, { schedule: { demo: 20 }, forceWebgl: true, density: ${JSON.stringify(density)} }).catch((err) => ((window.refused = { name: err.name, message: err.message, untouched: root.innerHTML === before }), null));</script>`;

async function open(page: Page, density: string): Promise<void> {
  await page.route("http://chart.example/**", async (route) => {
    const path = new URL(route.request().url()).pathname.slice(1);
    if (path === "host.html") return route.fulfill({ contentType: "text/html", body: host(density) });
    return route.fulfill({ response: await route.fetch({ url: base + path + new URL(route.request().url()).search }) });
  });
  await page.goto("http://chart.example/host.html");
  await page.waitForFunction("window.api !== undefined", undefined, { timeout: 180_000 });
}

for (const [density, height] of [["compact", 20], ["normal", 24], ["roomy", 40]] as const) {
  test(`with density ${density} the view state, the root's token, the table rows and the bars all read ${height} px`, async ({ page }) => {
    await open(page, density);
    await page.waitForFunction("window.gantt?.ready && window.gantt.status().frames >= 2", undefined, { timeout: 60_000 });
    const read = await page.evaluate(() => {
      const root = document.getElementById("g")!;
      // A summary bar is taller than a task bar, so only the bars of the commonest height are compared: the nearest two sit on neighbouring rows.
      const all = (window.gantt!.app.test_bar_rects() as { y: number; height: number }[]).map((bar) => ({ y: bar.y / window.devicePixelRatio, height: bar.height }));
      const count = (height: number): number => all.filter((bar) => bar.height === height).length;
      const common = all.map((bar) => bar.height).reduce((best, height) => (count(height) > count(best) ? height : best));
      const ys = all.filter((bar) => bar.height === common).map((bar) => bar.y).sort((a, b) => a - b);
      const pitch = Math.min(...ys.slice(1).map((y, i) => y - ys[i]!).filter((d) => d > 0.5));
      return { view: window.gantt!.status().rowHeight, token: getComputedStyle(root).getPropertyValue("--gantt-row-h").trim(), row: root.querySelector<HTMLElement>(".row")!.getBoundingClientRect().height, pitch, bars: ys.length };
    });
    expect(read.bars).toBeGreaterThan(3);
    expect(read).toMatchObject({ view: height, token: `${height}px`, row: height, pitch: height });
  });
}

test("a density outside compact, normal and roomy rejects mount with a RangeError naming it, and the root is untouched", async ({ page }) => {
  await open(page, "tall");
  const refused = await page.evaluate("window.refused") as { name: string; message: string; untouched: boolean };
  expect(refused).toMatchObject({ name: "RangeError", untouched: true });
  for (const word of ["tall", "compact", "normal", "roomy"]) expect(refused.message).toContain(word);
});
