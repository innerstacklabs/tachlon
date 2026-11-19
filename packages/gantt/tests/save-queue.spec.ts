import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

// A host that saves through a function (decision 0022, rule 3.3): the chart sends it each commit in order, one at a time, and a `SaveRefused` puts the commit and those behind it back.
// The page mounts the real chart twice over: plain `mount` with a function `save`, and the React component, whose `save` prop is swapped between two edits.
const MODULE = `import { mount, SaveRefused } from "/src/index.ts";
import { createElement as h } from "react"; import { createRoot } from "react-dom/client"; import { Gantt } from "/src/react.tsx";
const wait = (ms) => new Promise((done) => setTimeout(done, ms)), log = [], schedule = { demo: 30 }, run = { open: 0, peak: 0, refuse: "", mounts: 0 };
let root;
const save = (name) => async (set) => {
  const day = set.changes[0].set.dataDate.slice(0, 10);
  log.push(name + ":" + day), run.peak = Math.max(run.peak, ++run.open);
  await wait(40);
  run.open--;
  if (day === run.refuse) throw new SaveRefused({ message: "locked by the planner" });
};
const edit = (...days) => days.forEach((day) => window.chart.commands.setDataDate(day));
const react = async (name) => { root ??= createRoot(document.getElementById("app")); root.render(h(Gantt, { schedule, forceWebgl: true, save: save(name), onReady: (chart) => ((window.chart = chart), run.mounts++) })); await wait(500); };
Object.assign(window, { log, run, edit, react, plain: async () => (window.chart = await mount(document.getElementById("app"), { schedule: { demo: 30 }, forceWebgl: true, save: save("f") })) });
window.ready = true;`;
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">import "/tests/save-queue-host.ts";</script></body></html>`;
const host = { name: "save-queue-host", resolveId: (id: string) => (id === "/tests/save-queue-host.ts" ? id : null), load: (id: string) => (id === "/tests/save-queue-host.ts" ? MODULE : null) };

let server: ViteDevServer;
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", plugins: [host], server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

async function open(page: Page): Promise<void> {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.ready === true", undefined, { timeout: 120_000 });
}
const sent = (page: Page): Promise<string[]> => page.evaluate("log.slice()");
const dataDate = (page: Page): Promise<string | null> => page.evaluate("chart.state().dataDate");

test("a function `save` gets three quick edits in order, and a refusal of the second puts the chart back to after the first and shows why", async ({ page }) => {
  await open(page);
  await page.evaluate("plain()");
  await page.evaluate("edit('2024-02-05', '2024-02-06', '2024-02-07')");
  await expect.poll(() => sent(page), "three edits made at once reach `save` in the order made").toEqual(["f:2024-02-05", "f:2024-02-06", "f:2024-02-07"]);
  expect(await page.evaluate("run.peak"), "each call ends before the next starts").toBe(1);
  expect(await dataDate(page)).toBe("2024-02-07");

  await page.evaluate("run.refuse = '2024-02-09', edit('2024-02-08', '2024-02-09', '2024-02-10')");
  await expect(page.locator("#edit-error-text"), "the chart says why").toHaveText("locked by the planner (2 edits put back)");
  await expect.poll(() => dataDate(page), "the refused edit and the one behind it are put back").toBe("2024-02-08");
  expect((await sent(page)).slice(3), "the edit behind the refused one is never sent").toEqual(["f:2024-02-08", "f:2024-02-09"]);
});

test("the React component calls the `save` it holds when the commit is sent, so a new function passed between two edits gets the second", async ({ page }) => {
  await open(page);
  await page.evaluate("react('a')");
  await page.evaluate("edit('2024-02-05')");
  await expect.poll(() => sent(page)).toEqual(["a:2024-02-05"]);
  await page.evaluate("react('b')");
  await page.evaluate("edit('2024-02-06')");
  await expect.poll(() => sent(page), "the second edit goes to the new function").toEqual(["a:2024-02-05", "b:2024-02-06"]);
  expect(await page.evaluate("run.mounts"), "the chart was not mounted again").toBe(1);
});
