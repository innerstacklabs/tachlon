import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
// gantt#1875: the words of the chart's own markup come from the host's `texts` bundle, by key; a key the bundle leaves out stays English.
const HOST = `<div id="g" class="gantt" style="width: 1000px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.mountWith = async (texts) => (window.api = await mount(document.getElementById("g"), { tasks: [{ id: 1, name: "Dig", start: "2026-11-02", duration: 3 }], forceWebgl: true, texts }));</script>`;
const fit = "#g [data-part=fit-button]";
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
test("a bundle changes the Fit button's text and title, and leaves the keys it omits in English", async ({ page }) => {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  const mountWith = (texts: Record<string, string> | undefined) => page.evaluate((t) => (window as unknown as { mountWith(t?: Record<string, string>): Promise<unknown> }).mountWith(t), texts);
  await mountWith(undefined);
  await expect(page.locator(fit), "no bundle: English").toHaveText("Fit");
  await expect(page.locator(fit)).toHaveAttribute("title", /^Fit: show the whole project/);
  await expect(page.locator("#g [data-part=table-head] .name"), "the Task head is found by its class, not its text").toHaveText("Task");
  await page.evaluate(() => (window as unknown as { api: { destroy(): void } }).api.destroy());
  await mountWith({ fit: "Anpassen", "fit-title": "Das ganze Projekt zeigen (f)", task: "Aufgabe", "move-dur-up": "Dauer nach oben" });
  await expect(page.locator(fit)).toHaveText("Anpassen");
  await expect(page.locator(fit)).toHaveAttribute("title", "Das ganze Projekt zeigen (f)");
  await expect(page.locator("#g [data-part=table-head] .name"), "a translated Task head keeps its class").toHaveText("Aufgabe");
  await expect(page.locator("#g [data-part=columns-up-dur]"), "an attribute key sets the aria-label").toHaveAttribute("aria-label", "Dauer nach oben");
  await expect(page.locator("#g [data-part=toolbar] [aria-label=Zoom]"), "a key the bundle omits stays English").toHaveCount(1);
  await server.close();
});
