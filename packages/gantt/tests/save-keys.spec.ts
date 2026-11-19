import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

// The rows a host passes and the key of a row the person makes (decision 0022, rule 5): with `save` every row needs a key, and the chart gives a new row one.
const MODULE = `import { mount } from "/src/index.ts";
const sets = [], slots = [];
const rows = (...keys) => ({ tasks: keys.map((key, i) => ({ key, id: String(i + 1), name: "Task " + i, start: "2026-11-02", duration: 2 })) });
const tryMount = async (options) => { try { return (window.chart = await mount(document.getElementById("app"), { forceWebgl: true, ...options })), "mounted"; } catch (e) { return e.message; } };
const save = async (set) => void sets.push(set);
Object.assign(window, { sets, slots, rows, save, tryMount });
window.ready = true;`;
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">import "/tests/save-keys-host.ts";</script></body></html>`;
const host = { name: "save-keys-host", resolveId: (id: string) => (id === "/tests/save-keys-host.ts" ? id : null), load: (id: string) => (id === "/tests/save-keys-host.ts" ? MODULE : null) };

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

test("with save, mount refuses a task with no key and two with one key (noKey, duplicateKey), and a mount with no save takes keyless rows", async ({ page }) => {
  await open(page);
  const mounted = (code: string): Promise<string> => page.evaluate(code);
  expect(await mounted("tryMount({ ...rows('a', undefined), save })"), "a row with no key").toMatch(/^noKey: .*task 1/);
  expect(await mounted("tryMount({ ...rows('a', 'a'), save })"), "two rows with one key").toMatch(/^duplicateKey: .*"a"/);
  expect(await mounted("tryMount({ ...rows('a', 'b'), save, keys: 'servers' })"), "a keys setting that is neither").toMatch(/keys is "chart" or "server"/);
  expect(await mounted("tryMount({ ...rows('a', undefined) })"), "no save: keyless rows as before").toBe("mounted");
});

test("with save, a row the person makes gets a key from newKey before its change set is written, and the chart holds it", async ({ page }) => {
  await open(page);
  await page.evaluate("tryMount({ ...rows('a', 'b'), save, newKey: () => 'made-1' })");
  await page.evaluate("chart.on('edit', (e) => slots.push(e.commits[0].changes[0].task)), chart.commands.select('a'), chart.commands.insertRow()");
  await expect.poll(() => page.evaluate("sets.length"), "the create reaches save").toBe(1);
  expect(await page.evaluate("sets[0].changes[0]"), "the create carries its key in key and in set").toMatchObject({ record: "task", op: "create", key: "made-1", set: { key: "made-1" } });
  expect(await page.evaluate("chart.commands.assignKey({ task: slots[0] }, 'other')"), "the row already holds the key").toMatchObject({ ok: false, reason: "hasKey" });
});
