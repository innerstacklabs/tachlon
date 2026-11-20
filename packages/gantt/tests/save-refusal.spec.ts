import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

// A save the host refuses (decision 0022, rule 3.5): the chart selects the row the refusal names, shows its message at the cell the refusal's field names, and dims every row whose commit is unsaved until its call settles.
const MODULE = `import { mount, SaveRefused } from "/src/index.ts";
const calls = [], errs = [];
const save = (set) => new Promise((resolve, reject) => calls.push({ set, resolve, reject }));
const tasks = ["a", "b", "c"].map((key, i) => ({ key, id: String(i + 1), name: "Task " + i, start: "2026-11-02", days: 2 }));
const chart = async (more) => ((window.chart = await mount(document.getElementById("app"), { forceWebgl: true, tasks, ...more })), window.chart.on("error", (e) => errs.push(e)));
const refuseLinks = { createLink: async () => { throw Object.assign(new Error("No such link"), { status: 409 }); } };
const namedLinks = { thrown: { createLink: async () => { throw new SaveRefused({ message: "No such link", key: "b" }); } }, body: { createLink: async () => new Response(JSON.stringify({ message: "No such link", key: "b" }), { status: 409 }) } };
Object.assign(window, { calls, errs, SaveRefused, start: () => chart({ fields: { tasks: { duration: "days" } }, save }), startLinks: (how) => chart({ newKey: () => "b", save: how ? namedLinks[how] : refuseLinks }) });
window.ready = true;`;
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">import "/tests/save-refusal-host.ts";</script></body></html>`;
const host = { name: "save-refusal-host", resolveId: (id: string) => (id === "/tests/save-refusal-host.ts" ? id : null), load: (id: string) => (id === "/tests/save-refusal-host.ts" ? MODULE : null) };

let server: ViteDevServer;
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", plugins: [host], server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

const row = (page: Page, n: number) => page.locator(`#table-rows .row[data-row="${n}"]`);
async function open(page: Page, start = "start()"): Promise<void> {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.ready === true", undefined, { timeout: 120_000 });
  await page.evaluate(start);
  await row(page, 2).waitFor({ timeout: 60_000 });
}
const edit = async (page: Page, n: number, value: string): Promise<void> => {
  await row(page, n).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill(value);
  await page.keyboard.press("Enter");
};
const dim = (page: Page): Promise<(string | null)[]> => Promise.all([0, 1, 2].map((n) => row(page, n).getAttribute("data-pending")));

test("a refusal that names a row and a field selects the row, shows its message at that cell, and ends the dimming; the next edit clears the message", async ({ page }) => {
  await open(page);
  await edit(page, 1, "9");
  await edit(page, 2, "8");
  await expect.poll(() => page.evaluate("calls.length"), "the first commit is sent, the second waits").toBe(1);
  await expect.poll(() => dim(page), "the rows of both unsaved commits are dimmed, the other is not").toEqual([null, "true", "true"]);
  await row(page, 0).click({ position: { x: 4, y: 4 } }); // another row is selected while the call is held, so a selected row 1 afterwards is the chart's doing

  await page.evaluate("calls[0].reject(new SaveRefused({ message: 'Too long', key: 'b', field: 'days' }))");
  await expect(page.locator("#edit-error-text")).toContainText("Too long");
  await expect.poll(() => dim(page), "nothing is waiting any more").toEqual([null, null, null]);
  await expect(row(page, 1), "the row the refusal names is selected").toHaveAttribute("data-selected", "true");
  await expect(row(page, 0)).not.toHaveAttribute("data-selected", "true");
  const note = row(page, 1).locator(".cell-error");
  await expect(note, "the message shows once, in the row the refusal names").toHaveText("Too long");
  const [cell, shown] = [await row(page, 1).locator(".dur").boundingBox(), await note.boundingBox()];
  expect(shown!.x, "it starts at the duration cell, the chart's name for the host's `days`").toBeGreaterThanOrEqual(cell!.x - 1);
  expect(shown!.x).toBeLessThan(cell!.x + cell!.width);

  await edit(page, 0, "5");
  await expect(page.locator("#table-rows .cell-error"), "the next edit clears it").toHaveCount(0);
});

test("the dimming comes off when a call saves, and a row the host marked with setPending stays marked", async ({ page }) => {
  await open(page);
  await page.evaluate("chart.setPending(['c'])");
  await expect.poll(() => dim(page)).toEqual([null, null, "true"]);
  await edit(page, 1, "9");
  await expect.poll(() => dim(page), "the host's mark and the chart's together").toEqual([null, "true", "true"]);
  await page.evaluate("calls[0].resolve()");
  await expect.poll(() => dim(page), "only the chart's mark goes").toEqual([null, null, "true"]);
});

test("a refusal whose field is an Object.prototype name draws no note and breaks no frame, and the row is still selected", async ({ page }) => {
  await open(page);
  await edit(page, 1, "9");
  await page.evaluate("calls[0].reject(new SaveRefused({ message: 'Odd field', key: 'b', field: 'constructor' }))");
  await expect(page.locator("#edit-error-text")).toContainText("Odd field");
  await expect(row(page, 1)).toHaveAttribute("data-selected", "true");
  await expect(page.locator("#table-rows .cell-error"), "no cell is named, so no note").toHaveCount(0);
  await page.waitForTimeout(500);
  expect(await page.evaluate("errs.map((e) => e.source)"), "no frame failed on the field").toEqual(["save"]);
});

for (const [how, key] of [["", undefined], ["thrown", "b"], ["body", "b"]] as const) test(`a refused link change selects no task, though the link's key is a task's key (${how || "no key named"})`, async ({ page }) => {
  await open(page, `startLinks(${how && JSON.stringify(how)})`);
  await row(page, 0).click({ position: { x: 4, y: 4 } });
  await page.evaluate("window.gantt.app.set_predecessors_text(2, '2')"); // a link into task c from task b, made with the key 'b', which task b also holds
  await expect.poll(() => page.evaluate("errs.map((e) => [e.source, e.reason, e.key])"), "the refusal arrives").toEqual([["save", "refused", key]]);
  await expect(row(page, 0), "the selection stays where the person put it").toHaveAttribute("data-selected", "true");
  await expect(row(page, 1)).not.toHaveAttribute("data-selected", "true");
});
