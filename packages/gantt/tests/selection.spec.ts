import type { Page, TestInfo } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { AXIS, countColour, decodePng, type Png } from "./support/pixels";

// The selection journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P6): a click on a critical bar selects its task and
// highlights its driving path, a click on a summary bar selects it with no driving path.
// One 50k page is shared by the whole file (ruling devteam-1-90): the tests run in order on
// it, each starting from the view the one before left (the page is zoomed out once, in beforeAll). Only the 50,000-task
// demo has the hand-known chain, so the file skips for any other N, with the line verify.mjs printed. verify.mjs ran the
// section in the 1280x800 window, as this page does. The expected chain is known for the 50,000-task demo (seed 20261020)
// and was found without the code under test. The demo is outline layout D: the 43,364
// leaf tasks are engine::generate(20261020, 43364) with their engine ids, and
// the other 6,636 tasks are link-free summaries that move no leaf. A scratch
// program (not the app's selection, and no engine CPM call for the chain)
// rebuilt every early start from the generated tasks, calendars and links with
// a minute-by-minute calendar and its own forward pass; all 43,364 starts equal
// the engine's. A link drives its successor when it alone sets the successor's
// start (within a minute). From that (the demo's project shape, gs#320, replaced the 1,161-driver chain
// of the single-chain shape; the numbers were rebuilt with the same walk as the app's, nearest first):
// task 400 (critical) is driven back through 107 tasks to task 10443, which has no driving predecessor.
// Task 400's nearest driver is 42641. No task of this shape has more than 2,000 driving ancestors, so
// the cap is covered natively by crates/app/tests/select.rs (the_chain_is_capped_at_two_thousand).

import { ROW } from "./support/pixels";
const GOLDEN = { task: 400, head: 10443, firstDriver: 42641, minBack: 107 };
const ACCENT = [0x1b, 0x5b, 0xbd] as const; // the light `sel-edge` (tl#1193)

/** `App.test_selection_json()` once parsed: the selected task and its driving lists, or null. */
interface SelectionJson {
  task: number;
  back: number[];
  forward: number[];
  backCapped: boolean;
}

/** A `table_rows` row: the task first, then columns this journey does not read. */
type Row = [number, ...unknown[]];

interface SummaryAt {
  row: number;
  task: number;
  level: number;
  from: number;
  leaves: number;
  links: { pred: number; succ: number; predStart: number; succStart: number }[];
}

const nextFrame = (page: Page, k = 2): Promise<void> =>
  page.evaluate(
    (frameCount) =>
      new Promise<void>((done) => {
        const target = window.gantt!.status().frames + frameCount;
        const wait = (): void => (window.gantt!.status().frames >= target ? done() : void requestAnimationFrame(wait));
        requestAnimationFrame(wait);
      }),
    k,
  );

const status = (page: Page) => page.evaluate(() => window.gantt!.status());

// Skipped for any other N, with the line verify.mjs printed (the only skip the section had).
test.skip(n !== 50000, `skip click-to-select check: its hand-known chain is for N=50000 (N=${n})`);

test.describe.serial("selection journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };

  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    logs = trackLogs(page);
    const query = new URLSearchParams({ n: String(n) });
    if (backend === "webgl") query.set("backend", "webgl");
    await page.goto(`${url}?${query}`);
    await waitReady(page, { frames: 30, timeout: 180_000 });
    const bounds = await page.locator("#timeline").boundingBox();
    if (!bounds) throw new Error("#timeline has no bounding box");
    box = bounds;
    // Zoom out so the bars are one pan of a few dozen wheel steps away.
    let zoomed = 0;
    while ((await status(page)).pxPerMinute > 0.01 && zoomed < 80) {
      await zoomStep(120);
      zoomed++;
    }
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  // One ctrl+wheel step at the page's centre column, on the row ten below the axis.
  async function zoomStep(dy: number): Promise<void> {
    const px = box.x + box.width / 2;
    const py = box.y + AXIS + 10 * ROW;
    await page.mouse.move(px, py);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, dy);
    await page.keyboard.up("Control");
    await nextFrame(page);
  }

  async function shot(testInfo: TestInfo, name: string): Promise<Png> {
    await nextFrame(page);
    const png = await page.locator("#timeline").screenshot();
    await testInfo.attach(name, { body: png, contentType: "image/png" });
    return decodePng(png);
  }

  const selection = (): Promise<SelectionJson | null> =>
    page.evaluate(() => JSON.parse(window.gantt!.app.test_selection_json()) as SelectionJson | null);
  const markedRow = (): Promise<string | null> =>
    page.evaluate(() => document.querySelector<HTMLElement>('#table-rows .row[data-selected="true"]')?.dataset.row ?? null);

  // Scroll `row` to the top and pan until instant `es` is a fifth of the way across.
  async function revealAt(row: number, es: number): Promise<void> {
    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), Math.max(0, (row - 5) * ROW));
    await page.mouse.move(box.x + box.width / 2, box.y + AXIS + 10 * ROW);
    let off0 = Infinity;
    for (let i = 0; i < 200; i++) {
      await nextFrame(page);
      const dx = (await page.evaluate((t) => window.gantt!.app.x_of(t), es)) - box.width / 5;
      if (Math.abs(dx) < 1 || Math.abs(dx) >= off0) break; // there, or clamped
      off0 = Math.abs(dx);
      await page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 2000), 0);
    }
    await nextFrame(page, 2);
  }

  async function reveal(task: number): Promise<{ row: number; es: number | null }> {
    const found = await page.evaluate((t) => {
      const app = window.gantt!.app;
      const row = (JSON.parse(app.table_rows(0, app.task_count())) as Row[]).findIndex((r) => r[0] === t);
      return { row, es: row < 0 ? null : (app.row_start(row) ?? null) };
    }, task);
    if (found.row < 0) return found;
    // verify.mjs passed the start through unchecked; a missing one is a failure here.
    if (found.es === null) throw new Error(`task ${task} on row ${found.row} has no early start`);
    await revealAt(found.row, found.es);
    return found;
  }

  async function clickBar(task: number): Promise<{ row: number; es: number | null }> {
    const found = await reveal(task);
    expect.soft(found.row >= 0, `task ${task} is on a table row (${found.row})`).toBe(true);
    // Without a row there is no bar to click; stop here as the missing start would have.
    if (found.row < 0 || found.es === null) throw new Error(`task ${task} is not on a table row`);
    const s = await status(page);
    const x = box.x + (await page.evaluate((t) => window.gantt!.app.x_of(t), found.es)) + 1;
    const y = box.y + AXIS + (found.row * ROW - s.scrollY) + ROW / 2;
    await page.mouse.click(x, y);
    await nextFrame(page, 3);
    return found;
  }

  test("a click on a critical bar selects it and draws its driving path, and Escape or a click on empty space clears it", { tag: "@smoke" }, async ({}, testInfo) => {
    const found = await clickBar(GOLDEN.task);
    const sel = await selection();
    expect.soft(sel?.task === GOLDEN.task, `a click on a bar selects its task (${sel?.task}, want ${GOLDEN.task})`).toBe(true);
    expect.soft(sel?.back.includes(GOLDEN.head) === true, `the selection lists the hand-known chain head ${GOLDEN.head}`).toBe(true);
    expect.soft(sel?.back[0] === GOLDEN.firstDriver, `its nearest driver is ${GOLDEN.firstDriver} (${sel?.back[0]})`).toBe(true);
    expect
      .soft(
        sel !== null && sel.back.length >= GOLDEN.minBack && !sel.backCapped,
        `the chain holds at least ${GOLDEN.minBack} drivers, uncapped (${sel?.back.length})`,
      )
      .toBe(true);
    const st1 = await status(page);
    expect
      .soft(st1.selected?.task === GOLDEN.task && st1.selected.backCount === sel?.back.length, "the status reports the selection")
      .toBe(true);
    expect.soft((await markedRow()) === String(found.row), `the table marks the selected row ${found.row}`).toBe(true);
    const picked = countColour(await shot(testInfo, "select-critical"), ACCENT, 6);
    expect.soft(picked > 20, `the driving path is drawn in the accent colour (${picked} pixels)`).toBe(true);
    await page.keyboard.press("Escape");
    await nextFrame(page, 3);
    expect.soft((await selection()) === null && (await markedRow()) === null, "Escape clears the selection and the table mark").toBe(true);
    // A click on empty space (the axis band) clears it too.
    await clickBar(GOLDEN.task);
    expect.soft((await selection())?.task === GOLDEN.task, "the bar is selected again").toBe(true);
    await page.mouse.click(box.x + box.width / 2, box.y + AXIS / 2);
    await nextFrame(page, 3);
    expect.soft((await selection()) === null, "a click on empty space clears the selection").toBe(true);
  });

  test("a click on a summary bar selects it with no driving path, marks its row and draws its subtree in the accent", async ({}, testInfo) => {
    // A short click on a summary bar selects the summary (gs#74), marks its row and draws its subtree in the
    // accent; it has no driving path. Leaf ids are 0..43363 and summary ids 43364.. (decision 0002). A summary
    // span starts at the earliest start of its leaves, and its subtree is the run of rows after it with a deeper level.
    const SUMMARY_FIRST = 43364;
    const summaryAt: SummaryAt | null = await page.evaluate((first) => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, 60)) as Row[];
      const row = rows.findIndex((entry) => entry[0] >= first);
      if (row < 0) return null;
      const level = app.task_outline_level(rows[row][0]);
      const all = JSON.parse(app.table_rows(row + 1, app.task_count())) as Row[];
      const kids: { id: number; row: number }[] = [];
      let from = Infinity; // the leaves under the summary, then the first link between two of them
      for (let i = 0; i < all.length && app.task_outline_level(all[i][0]) > level; i++) {
        if (all[i][0] < first) {
          kids.push({ id: all[i][0], row: row + 1 + i });
          from = Math.min(from, app.row_start(row + 1 + i)!);
        }
      }
      // A link between two leaves of the subtree: the predecessor's hover card lists the successor's name, and the successor's lists the predecessor's.
      const details = (p: { id: number }) => app.task_details(p.id);
      // The nearest such pairs by rows: with work packages in parallel (gs#320) the first pair found can be off screen
      // or a sliver, so the checks below accept any of a dozen.
      const links = kids
        .flatMap((p) => kids.map((q) => [p, q] as const))
        .filter(
          ([p, q]) =>
            p !== q && details(p).successors.some((s: string) => s.includes(app.task_name(q.id))) && details(q).predecessors.some((s: string) => s.includes(app.task_name(p.id))),
        )
        .sort((a, b) => Math.abs(a[0].row - a[1].row) - Math.abs(b[0].row - b[1].row))
        .slice(0, 12)
        .map(([p, q]) => ({ pred: p.row, succ: q.row, predStart: app.row_start(p.row)!, succStart: app.row_start(q.row)! }));
      return { row, task: rows[row][0], level, from, leaves: kids.length, links };
    }, SUMMARY_FIRST);
    expect
      .soft(
        summaryAt !== null && summaryAt.leaves > 0,
        `a summary task is in the first 60 rows with leaves under it (${JSON.stringify(summaryAt)})`,
      )
      .toBe(true);
    // Without a summary there is nothing to click; the rest of the test is that block.
    if (summaryAt === null || summaryAt.leaves <= 0) return;
    await revealAt(summaryAt.row, summaryAt.from);
    const summaryStatus = await status(page);
    const summaryY = box.y + AXIS + (summaryAt.row * ROW - summaryStatus.scrollY) + ROW / 2;
    const summaryX = box.x + (await page.evaluate((t) => window.gantt!.app.x_of(t), summaryAt.from)) + 1;
    await page.mouse.move(summaryX, summaryY);
    await nextFrame(page, 2);
    expect
      .soft(
        (await status(page)).hover?.task === summaryAt.task,
        `the pointer is over summary task ${summaryAt.task} on row ${summaryAt.row} (hover ${JSON.stringify((await status(page)).hover?.task)})`,
      )
      .toBe(true);
    await page.mouse.click(summaryX, summaryY);
    await nextFrame(page, 3);
    const summarySelection = await selection();
    expect
      .soft(
        summarySelection?.task === summaryAt.task && summarySelection.back.length === 0 && summarySelection.forward.length === 0,
        `a click on a summary bar selects it with no driving path (${JSON.stringify(summarySelection)})`,
      )
      .toBe(true);
    expect.soft((await markedRow()) === String(summaryAt.row), `the table marks the summary row ${summaryAt.row} (${await markedRow()})`).toBe(true);
    const selectedStatus = (await status(page)).selected;
    expect
      .soft(
        selectedStatus?.task === summaryAt.task && selectedStatus.backCount === 0 && selectedStatus.forwardCount === 0,
        `the status reports summary task ${summaryAt.task} with no drivers (${JSON.stringify(selectedStatus)})`,
      )
      .toBe(true);
    const summaryShot = await shot(testInfo, "select-summary");
    // the summary bar is accent on its own, so test a descendant bar and a link between two descendants, placed from their rows
    if (summaryAt.links.length === 0) expect.soft(false, "two leaves under the summary are joined by a link").toBe(true);
    else {
      const lit = (rect: readonly [number, number, number, number]) => countColour(summaryShot, ACCENT, 6, rect);
      const seen: string[] = [];
      let drawn = false;
      for (const lk of summaryAt.links) {
        const [xp, xs] = await page.evaluate(([a, b]) => [a, b].map((t) => window.gantt!.app.x_of(t)), [lk.predStart, lk.succStart]);
        const top = AXIS + lk.pred * ROW - summaryStatus.scrollY; // the link leaves its predecessor's row along the boundary toward the successor
        const bar = lit([xp, top + ROW / 2 - 3, xp + 2, top + ROW / 2 + 3]);
        const ym = top + (lk.succ > lk.pred ? ROW : 0);
        const arrow = lit([Math.min(xp, xs) - 7, ym - 2, Math.max(xp, xs) + 7, ym + 2]);
        seen.push(`${bar}/${arrow}`);
        if (bar > 0 && arrow > 0) {
          drawn = true;
          break;
        }
      }
      expect
        .soft(drawn, `a descendant bar and the link under it are drawn in the accent colour (accent pixels bar/link per candidate: ${seen.join(" ")})`)
        .toBe(true);
    }
    await page.keyboard.press("Escape");
    await nextFrame(page, 3);
    expect.soft((await selection()) === null && (await markedRow()) === null, "Escape clears the summary selection and the table mark").toBe(true);
  });
});
