import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";

// gantt#2087: a new `records` value reaches the chart that is mounted as `update(records)`, in place, from the React, Vue and Angular components alike. Each host (tests/react-events-host.ts, vue-host.ts,
// angular-host.ts) runs its wrapper on tests/react-fake-core.ts, which records each `mount` and `update`, and offers `put(props)`: the props it was given so far, with these on top, and what happened.
const HOSTS = { React: "react-events-host.ts", Vue: "vue-host.ts", Angular: "angular-host.ts" };
const a = [{ key: "k1", duration: 3 }], b = [{ key: "k2", duration: 4 }], c = [{ key: "k3", duration: 5 }];
let server: ViteDevServer;
test.beforeAll(async () => {
  const here = (name: string): string => fileURLToPath(new URL(name, import.meta.url));
  const alias = [{ find: /^\.\/index\.js$/, replacement: here("react-fake-core.ts") }, { find: /^@angular\/core$/, replacement: here("angular-fake-core.ts") }];
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", resolve: { alias }, server: { host: "127.0.0.1", port: 0 } });
  await server.listen();
});
test.afterAll(() => server.close());

async function open(page: Page, host: string): Promise<(props: object) => Promise<{ mounts: number; updates: unknown[]; results: unknown[] }>> {
  const base = server.resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: `<!doctype html><html><body><script type="module">import "/tests/${host}";</script></body></html>` }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.put");
  return (props) => page.evaluate(`put(${JSON.stringify(props)})`) as never;
}

for (const [name, host] of Object.entries(HOSTS)) {
  test(`${name}: a new records value calls update once on the mounted chart and onUpdate gets the answer, a refusal too, and a new tasks value still mounts the chart again`, async ({ page }) => {
    const put = await open(page, host);
    expect.soft(await put({ tasks: [], records: a }), "the first value is not applied").toEqual({ mounts: 1, updates: [], results: [] });
    expect.soft(await put({ records: b }), "a new value is one update and no second mount").toEqual({ mounts: 1, updates: [b], results: [{ ok: true, updated: 1 }] });
    expect.soft(await put({ tasks: [] }), "a new tasks value mounts again and sends nothing").toMatchObject({ mounts: 2, updates: [b] });
    const bad = [{ key: "bad" }];
    expect.soft(await put({ records: bad }), "a refusal reaches onUpdate").toMatchObject({ mounts: 2, updates: [b, bad], results: [{ ok: true }, { ok: false, reason: "unknownKey", key: "bad" }] });
  });

  test(`${name}: records that change while the chart starts are applied once, the latest, after it; records that come with a new tasks value are not applied`, async ({ page }) => {
    const put = await open(page, host);
    await page.evaluate("globalThis.gate = new Promise((done) => (window.release = done)), 0");
    await put({ tasks: [], records: a });
    await put({ records: b });
    expect.soft(await put({ records: c }), "no chart is mounted, so nothing is sent yet").toMatchObject({ mounts: 1, updates: [] });
    await page.evaluate("release()");
    expect.soft(await put({}), "the start settled: the latest value, once").toEqual({ mounts: 1, updates: [c], results: [{ ok: true, updated: 1 }] });
    expect.soft(await put({ tasks: [], records: b }), "the chart mounts again from tasks: records of that render are not applied").toMatchObject({ mounts: 2, updates: [c] });
  });
}
