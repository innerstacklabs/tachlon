import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
// gantt#1875: the names of the chart's columns and fields come from the host's `texts` bundle, and a translated column keeps its class: the chart finds a column by it, never by its text.
const HOST = `<div id="g" class="gantt" tabindex="0" style="width: 1000px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.mountWith = async (texts) => (window.api = await mount(document.getElementById("g"), { tasks: [{ id: 1, name: "Dig", start: "2026-11-02", duration: 3 }], forceWebgl: true, texts }));</script>`;
const BUNDLE = { start: "Inicio", finish: "Fin", "actual-start": "Inicio real", "actual-finish": "Fin real", remaining: "Restante", "percent-head": "% completo", "var-start": "ΔInicio", "task-name": "Tarea", duration: "Duración" };
const head = (cls: string) => `#g [data-part=table-head] .cols > .${cls}`;
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
test("with Start as Inicio the cell keeps class start, and the columns the table adds, the card and the editor use the bundle's names", async ({ page }) => {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.evaluate((t) => (window as unknown as { mountWith(t: unknown): Promise<unknown> }).mountWith(t), BUNDLE);
  await page.click("#g [data-part=progress-toggle]");
  await page.evaluate(() => (window as unknown as { api: { commands: { setBaseline(): unknown } } }).api.commands.setBaseline());
  const row = "#g [data-part=table-rows] .row";
  await expect(page.locator(`${row} > .start`), "the Start cell keeps its class and its date").toHaveText("2026-11-02 08:00");
  const heads: Record<string, string> = { start: "Inicio", finish: "Fin", "actual-start": "Inicio real", "actual-finish": "Fin real", remaining: "Restante", percent: "% completo", "var-start": "ΔInicio" };
  for (const [cls, text] of Object.entries(heads)) await expect(page.locator(head(cls)), `the ${cls} head`).toHaveText(text);
  await expect(page.locator(head("var-finish")), "a name the bundle omits stays English").toHaveText("ΔFinish");
  await page.locator(`${row} > .dur`).click();
  await expect(page.locator(`${row}[data-selected=true]`), "the click selected the row").toHaveCount(1);
  await page.mouse.move(5, 600);
  await page.keyboard.press("Alt+Enter");
  await expect(page.locator("#g [data-part=hover-card] .label").first(), "the card's first row is Start").toHaveText("Inicio");
  await expect(page.locator("#g [data-part=hover-card] .label", { hasText: "Duración" }), "the card names the duration by the bundle").toHaveCount(1);
  await page.keyboard.press("Escape");
  await page.locator(`${row} > .start`).dblclick();
  await expect(page.locator(`${row} input`), "the editor is named by its field").toHaveAttribute("aria-label", "Inicio: Dig");
  await server.close();
});
