import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
// gantt#1875: a chart whose locale is a right-to-left language sets `dir="rtl"` on its root; its table sits right of the timeline and its offsets and indent read from the right.
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
let server: ViteDevServer;
let base: string;
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  base = (await server.listen()).resolvedUrls!.local[0]!;
});
test.afterAll(() => server.close());
const HOST = `<div id="g" class="gantt" tabindex="0" style="width: 1200px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.api = await mount(document.getElementById("g"), { schedule: { demo: 20 }, forceWebgl: true, locale: "ar" });</script>`;

test("with locale ar the root is rtl, and the table, panel offset and name indent read from the right", async ({ page }) => {
  await page.route("http://chart.example/**", async (route) => {
    const path = new URL(route.request().url()).pathname.slice(1);
    if (path === "host.html") return route.fulfill({ contentType: "text/html", body: HOST });
    return route.fulfill({ response: await route.fetch({ url: base + path + new URL(route.request().url()).search }) });
  });
  await page.goto("http://chart.example/host.html");
  await page.waitForFunction("window.api !== undefined", undefined, { timeout: 180_000 });
  const read = await page.evaluate(() => {
    const root = document.getElementById("g")!;
    const left = (part: string): number => root.querySelector(`[data-part=${part}]`)!.getBoundingClientRect().left;
    return {
      dir: root.dir,
      indent: root.querySelector<HTMLElement>(".row > .name")!.style.paddingInlineStart,
      panelEnd: getComputedStyle(root.querySelector("[data-part=overlay]")!).insetInlineEnd,
      tableRightOfTimeline: left("table") > left("timeline"),
    };
  });
  expect(read).toEqual({ dir: "rtl", indent: read.indent, panelEnd: "12px", tableRightOfTimeline: true });
  expect(read.indent, "the name cell's indent is its inline-start padding").not.toBe("");
});
