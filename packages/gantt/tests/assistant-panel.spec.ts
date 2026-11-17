import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import type { GanttHook } from "../src/test-hook";
import { chromePath } from "./support/knobs";

// gantt#1875: the assistant's chat panel beside a chart. The model is a stub that answers only from the messages it is handed on that call: a request starts a held proposal, and
// the words it answers are the project finish the tool result in its prompt carries. No real model, key or address is called.
const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="plan" class="gantt" tabindex="0" style="width: 1200px; height: 420px"></div>
<div id="side" class="gantt"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  import { mountAssistantPanel } from "/src/assistant/panel.ts";
  const api = await mount(document.getElementById("plan"), { schedule: { demo: 30 }, forceWebgl: true });
  const send = async (messages) => {
    const last = messages.at(-1);
    if (last.role === "user") {
      const [task, days] = last.content.match(/\\d+/g).map(Number);
      return { content: "", toolCalls: [{ id: "c1", name: "propose", arguments: { kind: "duration", task, days } }] };
    }
    if (window.name === "fail") throw new Error("network");
    return { content: last.result.ok ? "The project would finish " + last.result.finishNow : "Refused: " + last.result.reason };
  };
  mountAssistantPanel(document.getElementById("side"), send, api.commands);
  window.chart = { api, hook: window.gantt };
</script></body></html>`;

declare global {
  interface Window {
    chart: { api: Gantt; hook: GanttHook };
  }
}

let server: ViteDevServer;
let base = "";

test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0, strictPort: false } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(async () => {
  await server.close();
});
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });

/** Open the page and send "lengthen task N to 9 days" for a task with no children; answer the task and a reader of its duration. */
async function ask(page: Page, answer = "The project would finish"): Promise<{ duration: () => Promise<string>; was: string }> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => window.chart?.api !== undefined, undefined, { timeout: 180_000 });
  const task = await page.evaluate(() => (JSON.parse(window.chart.hook.app.table_rows(0, 20)) as number[][]).find((row) => row[5] === 0)![0]!);
  const duration = (): Promise<string> => page.evaluate((t) => window.chart.hook.app.task_duration(t), task);
  const was = await duration();
  await expect(page.getByRole("button", { name: "Accept" }), "nothing is held before a message").toBeHidden();
  await page.getByLabel("Message").fill(`lengthen task ${task} to 9 days`);
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByRole("log")).toContainText(answer);
  expect(await duration(), "held apart: the chart has not changed yet").toBe(was);
  return { duration, was };
}

test("a message is answered in the live region, and Accept applies the held change to the chart", async ({ page }) => {
  const { duration, was } = await ask(page);
  await page.getByRole("button", { name: "Accept" }).click();
  await expect(page.getByRole("log")).toContainText("Change applied.");
  await expect(page.getByRole("button", { name: "Accept" })).toBeHidden();
  expect(await duration(), "the chart shows the change").not.toBe(was);
});

test("Reject drops the held change and leaves the chart as it was", async ({ page }) => {
  const { duration, was } = await ask(page);
  await page.getByRole("button", { name: "Reject" }).click();
  await expect(page.getByRole("log")).toContainText("Change dropped.");
  expect(await duration()).toBe(was);
  expect(await page.evaluate(() => window.chart.api.commands.acceptProposal()), "nothing is held now").toMatchObject({ ok: false, reason: "noProposal" });
});
test("a turn that fails after a proposal still shows Accept and Reject for the held change", async ({ page }) => {
  await page.addInitScript(() => void (window.name = "fail")); // the stub's `send` throws on its second call
  await ask(page, "The assistant could not answer.");
  await expect(page.getByRole("button", { name: "Accept" })).toBeVisible();
  await page.getByRole("button", { name: "Reject" }).click();
  expect(await page.evaluate(() => window.chart.api.commands.acceptProposal()), "nothing is held now").toMatchObject({ ok: false, reason: "noProposal" });
});
