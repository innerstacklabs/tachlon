import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { dpr, n, url } from "./support/knobs";
import { ROW } from "./support/pixels";
import { checkTable, tableSettled } from "./support/table";

// The table journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P9). One 50k page is shared by the file per ruling
// devteam-1-90, in the 1280x800 window verify.mjs used. A fresh page starts at the year level and row 0 where
// verify.mjs had panned to the day level before the table section; the table checks do not depend on the time view.

// An MS Project file whose tasks carry no progress, so the table keeps the columns it had.
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));

const status = (page: Page) => page.evaluate(() => window.gantt!.status());

test.describe.serial("table sync journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    // WebGL2 from the start: a WebGPU device lost mid-test reloads the page onto WebGL2 and the test would count the new document.
    const query = new URLSearchParams({ n: String(n), backend: "webgl" });
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    const bounds = await page.locator("#timeline").boundingBox();
    if (!bounds) throw new Error("#timeline has no bounding box");
    box = bounds;
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("the table holds the visible rows at the start, with a generated leaf name and its outline indent", async () => {
    const st = await checkTable(page, box, "table at the start of the pan view");
    expect
      .soft(
        st.t.name !== "" && !/^Task \d+$/.test(st.t.name),
        `first table name is a non-placeholder generated name (actual ${JSON.stringify(st.t.name)})`,
      )
      .toBe(true);
    const firstMeta = await page.evaluate((r) => {
      const [task, , , , , kind] = JSON.parse(window.gantt!.app.table_rows(r, r + 1))[0];
      return {
        task,
        name: window.gantt!.app.task_label(task),
        pad: (window.gantt!.app.task_outline_level(task) - 1) * 14 + (kind === 0 ? 14 : 0),
      };
    }, st.t.first!);
    expect
      .soft(
        st.t.name === firstMeta.name,
        `first table name is the leaf name app.task_label(${firstMeta.task}): DOM ${JSON.stringify(st.t.name)}, app ${JSON.stringify(firstMeta.name)}`,
      )
      .toBe(true);
    expect
      .soft(
        st.t.pad === firstMeta.pad,
        `first table padding-left matches task ${firstMeta.task}'s outline: DOM ${st.t.pad}px, app ${firstMeta.pad}px`,
      )
      .toBe(true);
  });

  test("a wheel over the table and a jump of its scrollTop move the canvas", async ({}, testInfo) => {
    const y0 = (await status(page)).scrollY;
    await page.mouse.move(100, 400);
    await page.mouse.wheel(0, ROW * 400);
    await page
      .waitForFunction(([y, d]) => window.gantt!.status().scrollY >= y + d - 1, [y0, ROW * 400], { timeout: 10_000 })
      .catch(() => {});
    let st = await checkTable(page, box, "wheel over the table");
    expect
      .soft(
        st.s.scrollY - y0 >= ROW * 400 - 1,
        `wheel over the table scrolled the canvas ${st.s.scrollY - y0} px (want ${ROW * 400})`,
      )
      .toBe(true);
    const jumpRow = Math.floor(n * 0.6);
    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), ROW * jumpRow);
    st = await checkTable(page, box, `table scrolled to row ${jumpRow}`);
    expect
      .soft(
        st.s.rows[0] === jumpRow,
        `table scrolled to row ${jumpRow}: the canvas shows rows ${st.s.rows.join("-")}`,
      )
      .toBe(true);
    await testInfo.attach("5-table", { body: await page.screenshot(), contentType: "image/png" });
  });

  test("a wheel over the canvas and the c key move the table", async () => {
    const y1 = (await status(page)).scrollY;
    await page.mouse.move(box.x + box.width / 2, box.y + 300);
    await page.mouse.wheel(0, ROW * 100);
    let st = await checkTable(page, box, "wheel over the canvas");
    expect
      .soft(
        Math.abs(st.s.scrollY - y1 - ROW * 100) <= 1,
        `wheel over the canvas scrolled ${st.s.scrollY - y1} px (want ${ROW * 100})`,
      )
      .toBe(true);
    const y2 = st.s.scrollY;
    await page.keyboard.press("c");
    st = await checkTable(page, box, "c key");
    expect.soft(st.s.scrollY !== y2, `c scrolled the canvas to ${st.s.scrollY} and the table followed`).toBe(true);
  });

  test("the bound holds at the end of the schedule and after the window shrinks to 320 px", async () => {
    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), ROW * 1e6);
    let st = await checkTable(page, box, "table scrolled to the end");
    expect
      .soft(
        st.s.rows[1] === st.s.n && st.t.last === st.s.n - 1,
        `the end of the schedule: rows ${st.s.rows.join("-")} of ${st.s.n}, last DOM row ${st.t.last}`,
      )
      .toBe(true);
    const tall = st.s.rows[1] - st.s.rows[0];
    await page.setViewportSize({ width: 1280, height: 320 });
    await page
      .waitForFunction(
        (t) => window.gantt!.status().rows[1] - window.gantt!.status().rows[0] < t,
        tall,
        { timeout: 10_000 },
      )
      .catch(() => {});
    st = await checkTable(page, box, "after the window shrinks to 320 px");
    expect
      .soft(
        st.s.rows[1] - st.s.rows[0] < tall,
        `the canvas shrank with the window: ${tall} visible rows to ${st.s.rows[1] - st.s.rows[0]}`,
      )
      .toBe(true);
  });

  test("the idle window opens on a settled table after a scroll jump and after a document swap", async () => {
    // Case B: test 4 left the table at the end of the schedule. A jump of scrollTop to 0 owes the table one redraw, which
    // lands a frame or two later; the idle window opens only once the table has caught up.
    await page.waitForFunction(() => {
      const s = window.gantt!.status();
      const rows = document.querySelectorAll<HTMLElement>("#table-rows .row[data-row]");
      return s.rows[0] > s.n - 100 && rows.length > 0 && Number(rows[rows.length - 1].dataset.row) >= s.rows[1] - 1;
    });
    await page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 0));
    expect.soft(await tableSettled(page), "writes to the table over 30 frames after a jump of scrollTop to 0").toBe(0);

    // Case A: the WebGPU to WebGL2 fallback swaps the document, and the new one draws its table once more after it is ready.
    await page.evaluate(
      () => location.replace(`${location.pathname}?n=${new URLSearchParams(location.search).get("n")}&backend=webgl&fallback=GPU%20device%20lost%20(Unknown)`),
    );
    await page.waitForURL(/fallback=/, { waitUntil: "commit" });
    expect.soft(await tableSettled(page), "writes to the table over 30 frames in the swapped document").toBe(0);
  });

  test("the progress columns read what the engine holds", async () => {
    // The demo carries a percent complete but the engine holds no actuals, so the table keeps its five columns. The cells' text is
    // pinned by crates/app/tests/progress_text.rs; table-layout.spec.ts opens one-project.xer, which has actuals, and checks the head.
    await page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 0));
    await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
    const read = () =>
      page.evaluate(() => ({
        head: [...document.querySelectorAll("#table-head .cols > span")].map((cell) => cell.textContent ?? ""),
        classes: [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => [...row.children].map((cell) => cell.className)),
        flag: document.getElementById("table")!.dataset.progress,
        has: window.gantt!.app.has_progress(),
      }));
    const demo = await read();
    expect(demo.has).toBe(false);
    expect(demo.flag).toBeUndefined();
    expect(demo.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
    expect(demo.classes.length).toBeGreaterThan(5);
    for (const classes of demo.classes) expect(classes).toEqual(["name", "dur", "start", "finish", "float"]);

    // Idle, the table writes nothing: the sync draws the rows again only when the window or the schedule changes.
    const writes = await tableSettled(page);
    expect(writes, "writes to the table over 30 idle frames").toBe(0);

    // A file with no progress keeps the table it had.
    await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
    await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
    const plain = await read();
    expect(plain.has).toBe(false);
    expect(plain.flag).toBeUndefined();
    expect(plain.head).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
    for (const classes of plain.classes) expect(classes).toEqual(["name", "dur", "start", "finish", "float"]);
  });
});
