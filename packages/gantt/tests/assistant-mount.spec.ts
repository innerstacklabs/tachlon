import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { expect, test } from "@playwright/test";
import type { Gantt } from "../src/index";
import type { GanttHook } from "../src/test-hook";
import { chromePath } from "./support/knobs";

// gantt#1875: `mount`'s `assistant` option writes the chat panel into the host's element and runs each message through the host's `send`; `destroy` (the call a switch to WebGL2 makes on the old start) takes the panel out.
// `send` is a stub that answers only from the messages it is handed on that call. No real model, key or address is called.
const HOST = `<div id="plan" class="gantt" tabindex="0" style="width: 1200px; height: 420px"></div><div id="side"></div><script type="module">
  import { mount } from "/src/index.ts";
  const send = async (messages) => {
    const last = messages.at(-1);
    if (last.role === "user") {
      const [task, days] = last.content.match(/\\d+/g).map(Number);
      return { content: "", toolCalls: [{ id: "c1", name: "propose", arguments: { kind: "duration", task, days } }] };
    }
    return { content: last.result.ok ? "The project would finish " + last.result.finishNow : "Refused: " + last.result.reason };
  };
  const api = await mount(document.getElementById("plan"), { schedule: { demo: 30 }, forceWebgl: true, assistant: { send, into: document.getElementById("side") } });
  window.chart = { api, hook: window.gantt };</script>`;
declare global {
  interface Window {
    chart: { api: Gantt; hook: GanttHook };
  }
}
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
test("a mount with an assistant answers a message in the host's element, applies the accepted change, and leaves the element empty after destroy", async ({ page }) => {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => window.chart?.api !== undefined, undefined, { timeout: 180_000 });
  const scope = await page.locator("#side [data-part=assistant]").evaluate((el) => ({ scoped: el.closest(".gantt") !== null, display: getComputedStyle(el).display }));
  expect(scope, "the panel sits in the chart's styling scope, so assistant.css applies to it").toEqual({ scoped: true, display: "flex" });
  const task = await page.evaluate(() => (JSON.parse(window.chart.hook.app.table_rows(0, 20)) as number[][]).find((row) => row[5] === 0)![0]!);
  const duration = (): Promise<string> => page.evaluate((t) => window.chart.hook.app.task_duration(t), task);
  const was = await duration();
  await page.getByLabel("Message").fill(`lengthen task ${task} to 9 days`);
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.locator("#side").getByRole("log")).toContainText("The project would finish");
  expect(await duration(), "held apart: the chart has not changed yet").toBe(was);
  await page.getByRole("button", { name: "Accept" }).click();
  await expect(page.locator("#side").getByRole("log")).toContainText("Change applied.");
  expect(await duration(), "the chart shows the accepted change").not.toBe(was);
  await page.evaluate(() => window.chart.api.destroy());
  await expect(page.locator("#side"), "destroy takes the panel out of the host's element").toBeEmpty();
  await server.close();
});
