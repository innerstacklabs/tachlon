import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { assertExpandedSummary, firstTopLevelSummary, type FirstSummary } from "./support/outline";
import { checkTable } from "./support/table";

// The outline journey that verify.mjs (removed in 3f191ea6) used to run (gs#97 P8). One 50k page is shared by the file per ruling
// devteam-1-90, in the 1280x800 window verify.mjs used. A fresh page starts at the year level and row 0; verify had first
// scrolled the table to the c key's row, but the oracle finds the first top-level summary from either view.

import { ROW } from "./support/pixels";

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

test.describe.serial("outline journey", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: { x: number; y: number; width: number; height: number };
  let collapse: FirstSummary;

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
  });

  test.afterAll(async ({}, testInfo) => {
    await testInfo.attach("page-log", { body: logs.join("\n"), contentType: "text/plain" });
    const errors = pageErrors(logs);
    expect.soft(errors.length === 0, `no page errors${errors.length ? `: ${errors.join(" | ")}` : ""}`).toBe(true);
    await page.close();
  });

  test("collapsing and expanding the first top-level summary through its disclosure button", { tag: "@smoke" }, async () => {
    await page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 0));
    await nextFrame(page, 3);
    collapse = await firstTopLevelSummary(page);
    if (!collapse.hasSummary) {
      if (n === 50000) expect.soft(false, "collapse check: no summary row").toBe(true);
      else test.skip(true, "skip collapse check: no summary row");
      return;
    }
    if (collapse.r0 === undefined) {
      expect.soft(false, "top-level expanded summary row missing").toBe(true);
      return;
    }
    assertExpandedSummary(collapse);
    expect.soft(collapse.expected > 0, `summary ${collapse.task} subtree has ${collapse.expected} rows (want > 0)`).toBe(true);

    const disclosure = page.locator(`#table-rows .row[data-row="${collapse.r0}"] .disclosure`);
    if ((await disclosure.count()) === 0) {
      await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), collapse.r0 * ROW);
      await nextFrame(page, 3);
    }
    await page.locator(`#table-rows .row[data-row="${collapse.r0}"] .disclosure`).click();
    await page
      .waitForFunction((want) => window.gantt!.status().rowCount === want, collapse.before - collapse.expected, { timeout: 10_000 })
      .catch(() => {});
    await nextFrame(page, 3);
    const collapsed = await page.evaluate((r0) => {
      const app = window.gantt!.app;
      const s = window.gantt!.status();
      const row = document.querySelector<HTMLElement>(`#table-rows .row[data-row="${r0}"]`);
      return {
        rowCount: s.rowCount,
        n: s.n,
        spacer: document.getElementById("table-rows")!.getBoundingClientRect().height,
        kind: row?.dataset.kind ?? null,
        expanded: row?.querySelector(".disclosure")?.getAttribute("aria-expanded") ?? null,
        next: (JSON.parse(app.table_rows(r0 + 1, r0 + 2)) as number[][])[0]?.[0] ?? null,
      };
    }, collapse.r0);
    const collapsedCount = collapse.before - collapse.expected;
    expect.soft(collapsed.rowCount === collapsedCount, `collapsed rowCount ${collapsed.rowCount}, want ${collapsedCount}`).toBe(true);
    expect.soft(collapsed.n === n, `collapsed task count ${collapsed.n}, want ${n}`).toBe(true);
    expect.soft(Math.abs(collapsed.spacer - collapsed.rowCount * ROW) <= 1, `collapsed spacer ${collapsed.spacer}px, want ${collapsed.rowCount * ROW}px`).toBe(true);
    expect.soft(collapsed.kind === "2", `collapsed row kind ${collapsed.kind}, want 2`).toBe(true);
    expect.soft(collapsed.expanded === "false", `collapsed aria-expanded ${collapsed.expanded}, want false`).toBe(true);
    if (collapse.next !== null) {
      expect.soft(collapsed.next === collapse.next, `collapsed next task ${collapsed.next}, want ${collapse.next}`).toBe(true);
    }
    await page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 0));
    await checkTable(page, box, "collapsed first top-level row");

    await page.locator(`#table-rows .row[data-row="${collapse.r0}"] .disclosure`).click();
    await page.waitForFunction((want) => window.gantt!.status().rowCount === want, collapse.before, { timeout: 10_000 }).catch(() => {});
    await nextFrame(page, 3);
    const expanded = await page.evaluate((r0) => {
      const app = window.gantt!.app;
      const s = window.gantt!.status();
      const row = document.querySelector<HTMLElement>(`#table-rows .row[data-row="${r0}"]`);
      return {
        rowCount: s.rowCount,
        spacer: document.getElementById("table-rows")!.getBoundingClientRect().height,
        kind: row?.dataset.kind ?? null,
        expanded: row?.querySelector(".disclosure")?.getAttribute("aria-expanded") ?? null,
        firstChild: (JSON.parse(app.table_rows(r0 + 1, r0 + 2)) as number[][])[0]?.[0] ?? null,
      };
    }, collapse.r0);
    expect.soft(expanded.rowCount === collapse.before, `expanded rowCount ${expanded.rowCount}, want ${collapse.before}`).toBe(true);
    expect.soft(Math.abs(expanded.spacer - collapse.before * ROW) <= 1, `expanded spacer ${expanded.spacer}px, want ${collapse.before * ROW}px`).toBe(true);
    expect.soft(expanded.kind === "1", `expanded row kind ${expanded.kind}, want 1`).toBe(true);
    expect.soft(expanded.expanded === "true", `expanded aria-expanded ${expanded.expanded}, want true`).toBe(true);
    expect.soft(expanded.firstChild === collapse.firstChild, `expanded first child ${expanded.firstChild}, want ${collapse.firstChild}`).toBe(true);
    await checkTable(page, box, "re-expanded first top-level row");
  });

  test("Alt+Left and Alt+Right collapse, expand and move to the parent of the selected row", async () => {
    test.skip(collapse.r0 === undefined, "top-level expanded summary row missing");
    assertExpandedSummary(collapse);
    const altKeyPrevented = (key: string) =>
      page.evaluate((k) => {
        const ev = new KeyboardEvent("keydown", { key: k, altKey: true, bubbles: true, cancelable: true });
        document.getElementById("app")!.dispatchEvent(ev);
        return ev.defaultPrevented;
      }, key);
    const selectByName = async (task: number) =>
      page.evaluate((wanted) => {
        const app = window.gantt!.app;
        const matches = app.find(app.task_name(wanted));
        if (matches > 0) app.find_go(0);
        return { matches, selected: window.gantt!.status().selected?.task ?? null };
      }, task);
    await page.evaluate(() => window.gantt!.app.clear_selection());
    const noSelectionRowCount = (await status(page)).rowCount;
    expect.soft(await altKeyPrevented("ArrowLeft"), "Alt+Left without a selection prevents browser navigation").toBe(true);
    expect.soft(await altKeyPrevented("ArrowRight"), "Alt+Right without a selection prevents browser navigation").toBe(true);
    expect.soft((await status(page)).rowCount === noSelectionRowCount, "Alt+Left/Right without a selection leave rowCount unchanged").toBe(true);
    const summarySelection = await selectByName(collapse.task);
    expect.soft(summarySelection.matches > 0, `summary ${collapse.task} has a name match`).toBe(true);
    expect.soft(summarySelection.selected === collapse.task, `find selected summary ${summarySelection.selected}, want ${collapse.task}`).toBe(true);
    const beforeSelectedCollapse = (await status(page)).rowCount;
    await page.keyboard.press("Alt+ArrowLeft");
    await page.waitForFunction((before) => window.gantt!.status().rowCount < before, beforeSelectedCollapse, { timeout: 10_000 }).catch(() => {});
    const selectedCollapsed = await page.evaluate((task) => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as number[][];
      return { rowCount: rows.length, kind: rows.find((row) => row[0] === task)?.[5] ?? null };
    }, collapse.task);
    expect.soft(selectedCollapsed.rowCount < beforeSelectedCollapse, `Alt+Left reduces rowCount below ${beforeSelectedCollapse}`).toBe(true);
    expect.soft(selectedCollapsed.kind === 2, `Alt+Left changes summary ${collapse.task} kind to 2`).toBe(true);
    expect.soft(await altKeyPrevented("ArrowLeft"), "Alt+Left on an already-collapsed summary prevents browser navigation").toBe(true);
    expect.soft((await status(page)).rowCount === selectedCollapsed.rowCount, "Alt+Left on an already-collapsed summary leaves rowCount unchanged").toBe(true);

    await page.keyboard.press("Alt+ArrowRight");
    await page.waitForFunction((want) => window.gantt!.status().rowCount === want, beforeSelectedCollapse, { timeout: 10_000 }).catch(() => {});
    const selectedExpanded = await page.evaluate((task) => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as number[][];
      return { rowCount: rows.length, kind: rows.find((row) => row[0] === task)?.[5] ?? null };
    }, collapse.task);
    expect.soft(selectedExpanded.rowCount === beforeSelectedCollapse, `Alt+Right restores ${beforeSelectedCollapse} rows`).toBe(true);
    expect.soft(selectedExpanded.kind === 1, `Alt+Right changes summary ${collapse.task} kind to 1`).toBe(true);

    const leafPair = await page.evaluate((topSummary) => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as number[][];
      const at = rows.findIndex((row) => row[0] === topSummary);
      if (at < 0) return null;
      const topLevel = app.task_outline_level(topSummary);
      for (let r = at + 1; r + 1 < rows.length && app.task_outline_level(rows[r][0]) > topLevel; r++) {
        const level = app.task_outline_level(rows[r][0]);
        if (rows[r][5] === 1 && rows[r + 1][5] === 0 && app.task_outline_level(rows[r + 1][0]) === level + 1) {
          return { parent: rows[r][0], child: rows[r + 1][0] };
        }
      }
      return null;
    }, collapse.task);
    expect.soft(leafPair !== null, `summary ${collapse.task} contains an expanded summary with a leaf child`).toBe(true);
    if (leafPair !== null) {
      const leafSelection = await selectByName(leafPair.child);
      expect.soft(leafSelection.selected === leafPair.child, `find selected leaf ${leafSelection.selected}, want ${leafPair.child}`).toBe(true);
      expect.soft(await altKeyPrevented("ArrowRight"), "Alt+Right on a leaf prevents browser navigation").toBe(true);
      expect.soft((await status(page)).selected?.task === leafPair.child, `Alt+Right on leaf ${leafPair.child} leaves selection unchanged`).toBe(true);
      await page.keyboard.press("Alt+ArrowLeft");
      await page.waitForFunction((task) => window.gantt!.status().selected?.task === task, leafPair.parent, { timeout: 10_000 }).catch(() => {});
      expect.soft((await status(page)).selected?.task === leafPair.parent, `Alt+Left on leaf ${leafPair.child} selects parent ${leafPair.parent}`).toBe(true);
    }
    const all = (await status(page)).n;
    await page.keyboard.press("0");
    await page.waitForFunction((want) => window.gantt!.status().rowCount === want, all, { timeout: 10_000 }).catch(() => {});
    expect.soft((await status(page)).rowCount === all, `level shortcut checks end with all ${all} rows expanded`).toBe(true);
  });
});
