import type { Page } from "@playwright/test";
import { expect, pageErrors, test, trackLogs, waitReady } from "./support/gantt";
import { backend, dpr, n, url } from "./support/knobs";
import { assertExpandedSummary, firstTopLevelSummary, type FirstSummary } from "./support/outline";
import { AXIS } from "./support/pixels";

// The find journeys that verify.mjs (removed in 3f191ea6) used to run: the basic find journey (gs#97 P8), find under the critical-only
// filter (gs#97 P3), an edit made while find is open, and a bar drag made while find is open. One 50k page is shared by the
// whole file (ruling devteam-1-90): the tests run in order, and each starts from what the one before left.

import { ROW } from "./support/pixels";

interface FilterFind {
  query: string;
  matchingLeaves: number[];
  expectedAll: number;
  expectedFiltered: number;
  allCount: number;
  filteredCount: number;
  restoredCount: number;
  turnedOn: boolean;
  turnedOff: boolean;
  filterOff: boolean;
  findClosed: boolean;
}

interface ChangedFind {
  task: number;
  fresh: number;
}

interface BasicFind {
  query: string;
  expected: number[];
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

type Box = { x: number; y: number; width: number; height: number };

/**
 * Pan with plain wheel steps until a visible leaf row's bar starts a fifth of the way across, then zoom in with ctrl+wheel
 * steps at that start until the day level, where a bar is wide enough to press. verify.mjs reached this view through the
 * journeys before the find ones; here the page is at the year level and the find before has panned it away from the bars.
 */
async function zoomToDayAtBar(page: Page, box: Box): Promise<void> {
  const view = await status(page);
  const row = await page.evaluate(
    ([first, last]) => {
      for (let r = first + 1; r < last; r++) if ((window.gantt!.app.row_start(r) ?? 0) > 480) return r;
      return null;
    },
    view.rows,
  );
  expect(row, "a leaf row with a bar is in view").not.toBeNull();
  const es = (await page.evaluate((r) => window.gantt!.app.row_start(r), row!))!;
  const px = box.x + box.width / 5;
  const py = box.y + AXIS + row! * ROW - view.scrollY + ROW / 2;
  await page.mouse.move(px, py);
  let before = Infinity;
  for (let i = 0; i < 1000; i++) {
    const dx = (await page.evaluate((t) => window.gantt!.app.x_of(t), es)) - box.width / 5;
    if (Math.abs(dx) < 1 || Math.abs(dx) >= before) break; // there, or clamped
    before = Math.abs(dx);
    await page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 2000), 0);
    await nextFrame(page);
  }
  for (let i = 0; i < 200 && (await status(page)).level !== "day"; i++) {
    await page.mouse.move(px, py);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, -120);
    await page.keyboard.up("Control");
    await nextFrame(page);
  }
  expect((await status(page)).level, "reached the day tick level").toBe("day");
}
/**
 * The total in the find box's "N of M" text, 0 for "no matches", null for any other text. Another label can show for a
 * moment, for example while the page recovers from a lost GPU device (gs#494); null means "not settled yet", not a count.
 */
const parseFindCount = (text: string | null | undefined): number | null => {
  if (text === "no matches") return 0;
  const shape = /^[\d,]+ of ([\d,]+)$/.exec(text ?? "");
  return shape === null ? null : Number(shape[1].replaceAll(",", ""));
};
/** The total in a label that has already settled; NaN for any other text, so a comparison with a count fails. */
const countOf = (text: string | null): number => parseFindCount(text) ?? Number.NaN;
/** Waits for the find box to read exactly `want` matches. A label in another shape is waited out; a different count is never accepted. */
const waitForFindTotal = (page: Page, want: number): Promise<void> =>
  expect
    .poll(async () => parseFindCount(await page.locator("#find-count").textContent()), { timeout: 10_000, message: `find box reads ${want} matches` })
    .toBe(want);
/**
 * Waits until the find box reads the N of M or no matches shape and returns that text. Another label is waited out
 * (gs#594), so the caller reads one settled label rather than a transient one. A timeout names the last text it saw.
 */
const settledFindText = async (page: Page, timeout = 10_000): Promise<string> => {
  // One read before the poll's clock starts: on a busy machine the poll can time out before its first sample, and the error would name "".
  let text = (await page.locator("#find-count").textContent()) ?? "";
  try {
    await expect
      .poll(
        async () => {
          text = (await page.locator("#find-count").textContent()) ?? "";
          return parseFindCount(text);
        },
        { timeout, message: "find box settles on an N of M or no matches label" },
      )
      .not.toBeNull();
  } catch (error) {
    throw new Error(`find box did not settle on an N of M or no matches label; last text ${JSON.stringify(text)}`, { cause: error });
  }
  return text;
};

test("the find count parser reads only the N of M and no matches shapes", () => {
  expect(parseFindCount("3 of 1,234")).toBe(1234);
  expect(parseFindCount("no matches")).toBe(0);
  for (const transient of ["", "x", "went to date", "3 of", "of 5", "3 of 5 more", null]) {
    expect(parseFindCount(transient), `label ${JSON.stringify(transient)} is not a count`).toBeNull();
  }
});

test("the settled find label wait skips transient labels and returns the first count shape", async ({ browser }) => {
  for (const settled of ["2 of 5", "no matches"]) {
    const probe = await browser.newPage();
    try {
      await probe.setContent(`<span id="find-count">went to date</span>`);
      await probe.evaluate((final) => {
        const label = document.getElementById("find-count")!;
        setTimeout(() => void (label.textContent = ""), 150);
        setTimeout(() => void (label.textContent = "3 of"), 300);
        setTimeout(() => void (label.textContent = final), 450);
      }, settled);
      expect(await settledFindText(probe), `a transient label is waited out before ${JSON.stringify(settled)}`).toBe(settled);
    } finally {
      await probe.close();
    }
  }
});

test("a find label that never settles times out naming the last text it saw", async ({ browser }) => {
  const probe = await browser.newPage();
  try {
    await probe.setContent(`<span id="find-count">went to date</span>`);
    // A 1 ms poll ends before its first sample on any machine, so the name must come from the read before the poll.
    await expect(settledFindText(probe, 1)).rejects.toThrow(/last text "went to date"/);
  } finally {
    await probe.close();
  }
});

test.describe.serial("find journeys", () => { // oxlint-disable-line max-lines-per-function -- existing, over cap
  let page: Page;
  let logs: string[];
  let box: Box;
  let first: FirstSummary;
  let filterFind: FilterFind | null = null;
  let beforeTotal = 0;

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

  test("find selects the first match of a summary inside a collapsed summary, expands it, and counts the matches", { tag: "@smoke" }, async () => {
    first = await firstTopLevelSummary(page);
    if (!first.hasSummary) {
      if (n === 50000) expect.soft(false, "find check: no summary row").toBe(true);
      else test.skip(true, "skip find check: no summary row");
      return;
    }
    if (first.r0 === undefined) {
      test.skip(true, "skip find check: top-level expanded summary row missing");
      return;
    }
    assertExpandedSummary(first);
    const find = await page.evaluate((collapse): BasicFind => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, collapse.before)) as number[][];
      for (let r = collapse.r0 + 1; r <= collapse.r0 + collapse.expected; r++) {
        if (rows[r][5] !== 1) continue;
        const query = app.task_name(rows[r][0]).toUpperCase();
        const needle = query.toLowerCase();
        const expected = rows.map((row) => row[0]).filter((task) => app.task_name(task).toLowerCase().includes(needle));
        if (expected.length >= 3 && expected[0] === rows[r][0]) return { query, expected };
      }
      return { query: "", expected: [] };
    }, first);
    if (find.expected.length < 3) {
      test.skip(true, `skip find check: no summary inside the first top-level summary with 3 matches (${find.expected.length})`);
      return;
    }

    const t0Before = await page.evaluate(() => window.gantt!.app.time_at(0));
    await page.evaluate((task) => window.gantt!.app.toggle_task(task), first.task);
    await page
      .waitForFunction((want) => window.gantt!.status().rowCount === want, first.before - first.expected, { timeout: 10_000 })
      .catch(() => {});

    await page.keyboard.press("Control+f");
    expect.soft(await page.locator("#toolbar #find").isVisible(), "the find box is in the toolbar").toBe(true);
    expect.soft(await page.evaluate(() => document.activeElement?.id === "find-input"), "Ctrl+F focuses the find input").toBe(true);
    await page.keyboard.type(find.query);
    await page.waitForFunction((task) => window.gantt!.status().selected?.task === task, find.expected[0], { timeout: 10_000 }).catch(() => {});
    await nextFrame(page, 3);
    const found = await page.evaluate(() => {
      const s = window.gantt!.status();
      const row = document.querySelector<HTMLElement>('#table-rows .row[data-selected="true"]');
      return {
        selected: s.selected?.task ?? null,
        count: document.getElementById("find-count")!.textContent,
        rowCount: s.rowCount,
        rows: s.rows,
        selectedRow: s.selected?.row ?? null,
        markedTask: row?.dataset.task === undefined ? null : Number(row.dataset.task),
      };
    });
    expect.soft(found.selected === find.expected[0], `find selected task ${found.selected}, want ${find.expected[0]}`).toBe(true);
    expect
      .soft(found.count === `1 of ${find.expected.length}`, `find count ${JSON.stringify(found.count)}, want ${JSON.stringify(`1 of ${find.expected.length}`)}`)
      .toBe(true);
    expect.soft(found.rowCount === first.before, `find expanded rowCount ${found.rowCount}, want ${first.before}`).toBe(true);
    expect
      .soft(
        found.selectedRow !== null && found.selectedRow >= found.rows[0] && found.selectedRow < found.rows[1],
        `find selected row ${found.selectedRow} is inside ${found.rows.join("-")}`,
      )
      .toBe(true);
    expect.soft(found.markedTask === find.expected[0], `find marked table task ${found.markedTask}, want ${find.expected[0]}`).toBe(true);

    // Leave the page as the tests after this one expect it: the find box empty, nothing selected, the first view back.
    // Enter, Shift+Enter, a query with no match and Escape are step 5 of journey-j2.spec.ts.
    await page.keyboard.press("Escape");
    await page.evaluate(() => window.gantt!.app.clear_selection());
    await page.mouse.move(box.x + box.width / 2, box.y + 300);
    for (let i = 0; i < 200; i++) {
      await nextFrame(page);
      const px = (t0Before - (await page.evaluate(() => window.gantt!.app.time_at(0)))) * (await status(page)).pxPerMinute;
      if (Math.abs(px) < 0.5) break;
      await page.mouse.wheel(Math.sign(px) * Math.min(Math.abs(px), 1e6), 0);
    }
    await page.evaluate(() => (document.getElementById("table-body")!.scrollTop = 0));
    await nextFrame(page, 3);
  });

  test("find counts follow the critical-only filter, and leave the filter off and the find box empty", async ({}, testInfo) => {
    // Find follows the critical-only filter while collapse remains irrelevant. The expanded table supplies both the name
    // oracle and an independent filter oracle from leaf float plus ancestor levels.
    filterFind = await page.evaluate((): FilterFind | null => {
      const app = window.gantt!.app;
      if (app.critical_only()) app.set_critical_only(false);
      const rows = JSON.parse(app.table_rows(0, app.test_row_count())) as number[][];
      const names = rows.map((row) => app.task_name(row[0]).toLowerCase());
      const keep: boolean[] = Array(rows.length).fill(false);
      const summaries: { i: number; level: number }[] = [];
      for (let i = 0; i < rows.length; i++) {
        const [task, , , float, , kind] = rows[i];
        const level = app.task_outline_level(task);
        while (summaries.length && summaries[summaries.length - 1].level >= level) summaries.pop();
        if (kind !== 0) summaries.push({ i, level });
        if (kind === 0 && float === 0) {
          keep[i] = true;
          for (const summary of summaries) keep[summary.i] = true;
        }
      }

      const candidates = new Set([" / "]);
      for (const name of names) {
        for (const part of name.split("/")) {
          const segment = part.trim();
          if (segment.length >= 3) candidates.add(segment);
          for (const word of segment.split(/\s+/)) if (word.length >= 3) candidates.add(word);
        }
      }
      let picked: { query: string; matches: number[]; shown: number } | null = null;
      for (const query of candidates) {
        const matches = names.map((name, i) => (name.includes(query) ? i : -1)).filter((i) => i >= 0);
        const shown = matches.filter((i) => keep[i]).length;
        if (shown > 0 && shown < matches.length && (picked === null || matches.length < picked.matches.length)) {
          picked = { query, matches, shown };
        }
      }
      if (picked === null) return null;

      const allCount = app.find(picked.query);
      const turnedOn = app.set_critical_only(true);
      const filteredRows = JSON.parse(app.table_rows(0, app.test_row_count())) as number[][];
      const filteredExpected = filteredRows.filter((row) => app.task_name(row[0]).toLowerCase().includes(picked!.query)).length;
      const filteredCount = app.find(picked.query);
      const turnedOff = app.set_critical_only(false);
      const restoredCount = app.find(picked.query);
      app.find("");
      return {
        query: picked.query,
        matchingLeaves: picked.matches.filter((i) => rows[i][5] === 0).map((i) => rows[i][0]),
        expectedAll: picked.matches.length,
        expectedFiltered: filteredExpected,
        allCount,
        filteredCount,
        restoredCount,
        turnedOn,
        turnedOff,
        filterOff: !app.critical_only(),
        findClosed: (document.getElementById("find-input") as HTMLInputElement).value === "",
      };
    });
    if (filterFind === null) {
      const note = "skip filtered find check: no query matches both filter-shown and filter-hidden tasks";
      console.log(note);
      await testInfo.attach("skip-note", { body: note, contentType: "text/plain" });
      return;
    }
    expect
      .soft(filterFind.turnedOn && filterFind.turnedOff, `filtered find toggled the filter for ${JSON.stringify(filterFind.query)}`)
      .toBe(true);
    expect.soft(filterFind.allCount === filterFind.expectedAll, `unfiltered find count ${filterFind.allCount}, want ${filterFind.expectedAll}`).toBe(true);
    expect
      .soft(
        filterFind.filteredCount === filterFind.expectedFiltered,
        `critical-only find count ${filterFind.filteredCount}, want ${filterFind.expectedFiltered}`,
      )
      .toBe(true);
    expect
      .soft(filterFind.restoredCount === filterFind.expectedAll, `restored find count ${filterFind.restoredCount}, want ${filterFind.expectedAll}`)
      .toBe(true);
    expect.soft(filterFind.filterOff && filterFind.findClosed, "filtered find check leaves the filter off and the find box empty").toBe(true);
  });

  test("an edit made while find is open under the filter refreshes the count, and undo restores it", async () => {
    test.skip(filterFind === null, "no query matches both filter-shown and filter-hidden tasks");
    const found = filterFind!;

    await page.keyboard.press("Control+f");
    expect.soft(await page.evaluate(() => document.activeElement?.id === "find-input"), "filtered edit find focuses the find box through Ctrl+F").toBe(true);
    const filterTurnedOn = await page.evaluate(() => window.gantt!.app.set_critical_only(true));
    expect.soft(filterTurnedOn, "filtered edit find turns the critical-only filter on").toBe(true);
    await page.locator("#find-input").fill(found.query);
    const countBeforeEdit = await settledFindText(page);
    beforeTotal = countOf(countBeforeEdit);
    expect
      .soft(beforeTotal === found.expectedFiltered, `filtered edit find starts at ${countBeforeEdit}, want ${found.expectedFiltered} matches`)
      .toBe(true);

    const changedFind = await page.evaluate(
      ({ candidates, query, before }): ChangedFind | null => {
        const app = window.gantt!.app;
        const shown = new Set((JSON.parse(app.table_rows(0, app.test_row_count())) as number[][]).map((row) => row[0]));
        candidates.sort((a, b) => Number(shown.has(b)) - Number(shown.has(a)));
        for (const task of candidates) {
          const duration = `${app.test_task_minutes(task) + 1_000_000}m`;
          try {
            app.set_duration(task, duration);
          } catch {
            continue;
          }
          const rows = JSON.parse(app.table_rows(0, app.test_row_count())) as number[][];
          const fresh = rows.filter((row) => app.task_name(row[0]).toLowerCase().includes(query)).length;
          if (fresh !== before) return { task, fresh };
          app.undo();
        }
        return null;
      },
      { candidates: found.matchingLeaves, query: found.query, before: beforeTotal },
    );
    if (changedFind === null) {
      expect.soft(false, `no duration edit changes matches for ${JSON.stringify(found.query)}`).toBe(true);
    } else {
      await waitForFindTotal(page, changedFind.fresh);
      const countAfterEdit = await page.locator("#find-count").textContent();
      const afterTotal = countOf(countAfterEdit);
      expect.soft(afterTotal === changedFind.fresh, `filtered edit find refreshes to ${countAfterEdit}, want ${changedFind.fresh} matches`).toBe(true);
      expect.soft(afterTotal !== beforeTotal, `filtered edit find count changes from ${beforeTotal} to ${afterTotal}`).toBe(true);

      const undone = await page.evaluate(() => window.gantt!.app.undo());
      expect.soft(undone, `filtered edit find undoes the duration edit to task ${changedFind.task}`).toBe(true);
      await waitForFindTotal(page, beforeTotal);
      const countAfterUndo = await page.locator("#find-count").textContent();
      const undoTotal = countOf(countAfterUndo);
      expect.soft(undoTotal === beforeTotal, `filtered edit find returns to ${countAfterUndo}, want ${beforeTotal} matches after undo`).toBe(true);
    }
  });

  test("a bar drag made while find is open runs find once, after the drag ends, and undo restores the count", async () => {
    test.skip(filterFind === null, "no query matches both filter-shown and filter-hidden tasks");
    type FindProbe = typeof window & { __findCalls?: number };
    const restoreFind = () =>
      page.evaluate(() => {
        Reflect.deleteProperty(window.gantt!.app, "find");
        (window as FindProbe).__findCalls = undefined;
      });

    const dragCountText = await settledFindText(page);
    await zoomToDayAtBar(page, box);
    await page.evaluate(() => {
      const app = window.gantt!.app;
      (window as FindProbe).__findCalls = 0;
      const orig = app.find.bind(app);
      app.find = (q: string) => {
        (window as FindProbe).__findCalls! += 1;
        return orig(q);
      };
    });
    const dragStatus = await status(page);
    let dragPoint: { x: number; y: number } | null = null;
    for (let r = dragStatus.rows[0]; r < dragStatus.rows[1]; r++) {
      const x = box.x + (await page.evaluate((row) => window.gantt!.app.x_of(window.gantt!.app.row_start(row)!), r)) + 4;
      const y = box.y + AXIS + r * ROW - dragStatus.scrollY + ROW / 2;
      if (x < box.x + 8 || x > box.x + box.width - 80) continue;
      await page.mouse.move(x, y);
      await nextFrame(page, 2);
      if ((await status(page)).hover !== null) {
        dragPoint = { x, y };
        break;
      }
    }
    expect.soft(dragPoint !== null, "a bar is under the pointer to drag with find open").toBe(true);
    if (dragPoint === null) {
      await restoreFind();
    } else {
      const dragRevision = (await status(page)).revision;
      await page.mouse.down();
      for (let k = 1; k <= 8; k++) await page.mouse.move(dragPoint.x + 15 * k, dragPoint.y);
      await nextFrame(page, 3);
      const during = await status(page);
      const callsDuring = await page.evaluate(() => (window as FindProbe).__findCalls);
      const textDuring = await page.locator("#find-count").textContent();
      expect.soft(during.drag?.active === true, "the bar drag is active with find open").toBe(true);
      expect.soft(callsDuring === 0, `find ran ${callsDuring} times during the bar drag, want 0`).toBe(true);
      expect.soft(textDuring === dragCountText, `find count stays ${dragCountText} during the bar drag (got ${textDuring})`).toBe(true);

      await page.mouse.up();
      await nextFrame(page, 4);
      const dragAfter = await status(page);
      expect
        .soft(dragAfter.revision > dragRevision, `the find-open drag edited the schedule (revision ${dragRevision} to ${dragAfter.revision})`)
        .toBe(true);
      await page.waitForFunction(() => ((window as FindProbe).__findCalls ?? 0) >= 1, null, { timeout: 10_000 });
      await nextFrame(page, 4);
      const callsAfter = await page.evaluate(() => (window as FindProbe).__findCalls);
      expect.soft(callsAfter === 1, `one find refresh after the drag ends, got ${callsAfter}`).toBe(true);

      await page.keyboard.press("Control+Z");
      await nextFrame(page, 3);
      await restoreFind();
      await page.waitForFunction((text) => document.getElementById("find-count")!.textContent === text, dragCountText, {
        timeout: 10_000,
      });
      const dragCountAfterUndo = await page.locator("#find-count").textContent();
      expect.soft(dragCountAfterUndo === dragCountText, `find count returns to ${dragCountText} after undoing the drag`).toBe(true);
    }
    // The canvas press blurred the find input; Ctrl+F focuses it again so Escape closes the box.
    await page.keyboard.press("Control+f");
    await page.keyboard.press("Escape");
    await page.evaluate(() => window.gantt!.app.set_critical_only(false));
    await nextFrame(page, 3);
    expect
      .soft(!(await status(page)).criticalOnly && (await page.locator("#find-input").inputValue()) === "", "filtered edit find leaves the filter off and the find box empty")
      .toBe(true);
  });

  test("an empty search under a row filter says it searched the shown rows, and stays bare with no filter (gs#703)", async () => {
    // A name that only a row the filter hides holds: with Critical only on, find has nothing to count.
    const hiddenName = await page.evaluate((): string | null => {
      const app = window.gantt!.app;
      app.set_critical_only(false);
      const names = (JSON.parse(app.table_rows(0, app.test_row_count())) as number[][]).map((row) => app.task_name(row[0]).toLowerCase());
      app.set_critical_only(true);
      const found = names.find((name) => name.length >= 3 && app.find(name) === 0) ?? null;
      app.find("");
      return found;
    });
    expect(hiddenName, "a task name that only a critical-only-hidden row holds").not.toBeNull();
    await page.evaluate(() => window.gantt!.app.set_critical_only(true));
    await nextFrame(page, 3);
    await page.locator("#find-input").fill(hiddenName!);
    await expect(page.locator("#find-count")).toHaveText("no matches");
    await expect(page.locator("#find-count")).toHaveAttribute("title", "no matches in the shown rows");
    await page.evaluate(() => window.gantt!.app.set_critical_only(false));
    await nextFrame(page, 3);
    await page.locator("#find-input").fill("zzzz no such task");
    await expect(page.locator("#find-count")).toHaveText("no matches");
    await expect(page.locator("#find-count")).not.toHaveAttribute("title", /./);
    await page.locator("#find-input").fill("");
  });
});
