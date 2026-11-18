import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { test as base } from "@playwright/test";
import { chromePath } from "./support/knobs";
import { expect, test } from "./support/gantt";
import type { ViewState } from "../src/view-state";

// tl#1289: a host reads the open file's name, its unsaved mark and its import notes from `state()` and hears each flip as a `change`.
const file = (name: string) => ({ name, mimeType: "application/octet-stream", buffer: readFileSync(new URL(`./fixtures/${name}`, import.meta.url)) });

test("state() carries fileName, unsaved and importNotes, and change fires when unsaved flips", async ({ g }) => {
  await g.open({ n: 50 });
  await g.page.evaluate(() => {
    const seen: boolean[] = [];
    (window as unknown as { seen: boolean[] }).seen = seen;
    window.gantt!.api!.on("change", (s) => seen.push(s.unsaved));
  });
  const read = (): Promise<ViewState> => g.page.evaluate(() => window.gantt!.api!.state());
  expect(await read()).toMatchObject({ fileName: null, unsaved: false });

  await g.page.setInputFiles("#open-input", file("links.xml"));
  await g.page.waitForFunction(() => window.gantt!.opened?.name === "links.xml");
  await g.nextFrame(3);
  expect(await read()).toMatchObject({ fileName: "links.xml", unsaved: false });

  await g.page.evaluate(() => {
    (window as unknown as { seen: boolean[] }).seen.length = 0;
    window.gantt!.api!.commands.insertRow();
  });
  await g.nextFrame(3);
  expect((await read()).unsaved).toBe(true);
  await g.page.evaluate(() => window.gantt!.api!.download());
  await g.nextFrame(3);
  expect((await read()).unsaved).toBe(false);
  expect(await g.page.evaluate(() => (window as unknown as { seen: boolean[] }).seen.filter((u, i, a) => a[i - 1] !== u))).toEqual([true, false]);

  await g.page.setInputFiles("#open-input", file("one-project.xer"));
  await g.page.waitForFunction(() => window.gantt!.opened?.name === "one-project.xer");
  await g.nextFrame(3);
  const lines = await g.page.evaluate(() => document.querySelector("#unsupported-list")!.textContent!.split("\n"));
  const notes = (await read()).importNotes;
  expect(notes.length).toBeGreaterThan(0);
  expect([...notes]).toEqual(lines);
});

// A host that mounts with a File, then loads: the name and the notes follow the schedule, and a document has no file name.
const HOST = `<div id="g" class="gantt" style="width: 1000px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts";
  import { installChrome } from "/@id/@gantt/chrome";
  window.mountFile = async (name, bytes) => {
    window.api = await mount(document.getElementById("g"), { schedule: new File([new Uint8Array(bytes)], name), forceWebgl: true });
    installChrome(window.api, document.getElementById("g")); // the import notes panel is chrome
  };</script>`;
base.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
base("mount with a File and load({ file }) give its name and notes; load({ document }) gives no file name", async ({ page }) => {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const url = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${url}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${url}host.html`);
  const bytes = [...file("one-project.xer").buffer];
  await page.waitForFunction("window.mountFile !== undefined", undefined, { timeout: 180_000 });
  const facts = () => page.evaluate(async () => {
    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
    const { fileName, importNotes } = (window as any).api.state();
    return { fileName, notes: [...importNotes], panel: document.querySelector("#unsupported-list")!.textContent!.split("\n").filter(Boolean) };
  });
  await page.evaluate(([name, data]) => (window as any).mountFile(name, data), ["one-project.xer", bytes] as const);
  const mounted = await facts();
  expect(mounted.fileName).toBe("one-project.xer");
  expect(mounted.notes.length).toBeGreaterThan(0);
  expect(mounted.notes).toEqual(mounted.panel);
  await page.evaluate(() => (window as any).api.load({ document: (window as any).api.toDocument() }));
  const doc = await facts();
  expect(doc.fileName).toBeNull();
  expect(doc.notes).toEqual(doc.panel);
  await page.evaluate(async (data) => (window as any).api.load({ file: new File([new Uint8Array(data)], "again.xer") }), bytes);
  const again = await facts();
  expect(again).toMatchObject({ fileName: "again.xer" });
  expect(again.notes).toEqual(again.panel);
  await server.close();
});
