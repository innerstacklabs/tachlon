import type { Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";
import { checkTable } from "./support/table";

// Journey J2 ("J2. Navigate a large schedule"), walked on one page of the generated demo (50,000 tasks
// unless N says otherwise), one step per journey step that is built. Which rows the filters, the lookahead and find keep is
// checked on `Session` in crates/app/tests; these steps check that a reader reaches each one through the page. Step 2
// (filter or group by WBS or by an activity code) is not walked: grouping is not built, and the activity code filter needs a
// file with codes, which the demo is not (filters.spec.ts opens one).

/** A `table_rows` row: task, start, finish, float, critical, kind (0 is a leaf). */
type Row = [task: number, start: string, finish: string, float: number | null, critical: boolean, kind: number];

const fmt = new Intl.NumberFormat("en-US");
const allRows = (page: Page): Promise<Row[]> =>
  page.evaluate(() => JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as Row[]);
const rowCountIs = (page: Page, want: number) =>
  page.waitForFunction((w) => window.gantt!.status().rowCount === w, want, { timeout: 10_000 });
const filterIs = (page: Page, want: string) =>
  page.waitForFunction((w) => window.gantt!.status().filter === w, want, { timeout: 10_000 });
const selectedIs = (page: Page, task: number) =>
  page.waitForFunction((t) => window.gantt!.status().selected?.task === t, task, { timeout: 10_000 });

test("J2: the reader collapses the outline, filters, sets a lookahead, finds an activity and reads its driving path", async ({ g }) => {
  const { page } = g;
  await g.open();
  const box = await g.box();
  const { n } = await g.status();
  const count = page.locator("#row-count");

  await test.step("1. collapses the outline to level 1, then level 2, with the level keys, and 0 expands it all", async () => {
    const levels = await page.evaluate(() => {
      const app = window.gantt!.app;
      return Array.from({ length: app.task_count() }, (_, task) => app.task_outline_level(task));
    });
    const level1 = levels.filter((level) => level === 1).length;
    const level2 = levels.filter((level) => level <= 2).length;
    expect(level1, "the demo has an outline below level 1").toBeLessThan(level2);
    await page.keyboard.press("1");
    await rowCountIs(page, level1);
    const shown = (await allRows(page)).map((row) => row[0]);
    expect(await page.evaluate((tasks) => tasks.map((t) => window.gantt!.app.task_outline_level(t)), shown)).toEqual(shown.map(() => 1));
    await page.keyboard.press("2");
    await rowCountIs(page, level2);
    await page.keyboard.press("0");
    await rowCountIs(page, n);
  });

  await test.step("3. turns on longest path from the Filters menu and by Shift+L, one row filter at a time", async () => {
    await openFilters(page);
    await page.locator("#longest-toggle").click();
    await filterIs(page, "longest");
    const on = await g.status();
    expect(on.rowCount).toBeLessThan(n);
    await expect(page.locator("#longest-toggle")).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#critical-toggle")).toHaveAttribute("aria-pressed", "false");
    await expect(count).toHaveText(`Longest path · ${fmt.format(on.keptLeaves!)} of ${fmt.format(n)} activities`);
    await checkTable(page, box, "longest path");
    await openFilters(page);
    await page.locator("#critical-toggle").click();
    await filterIs(page, "critical");
    await expect(page.locator("#longest-toggle"), "critical only releases longest path").toHaveAttribute("aria-pressed", "false");
    await page.keyboard.press("Shift+L");
    await filterIs(page, "longest");
    await page.keyboard.press("Shift+L");
    await filterIs(page, "off");
    await expect(count).toHaveText(`${fmt.format(n)} rows`);
    await checkTable(page, box, "longest path off");
  });

  await test.step("4. sets a 3-week lookahead from a date, and the row count names it", async () => {
    const starts = (await allRows(page)).filter((row) => row[5] === 0).map((row) => row[1]).sort();
    const day = starts[Math.floor(starts.length / 2)]!.slice(0, 10);
    const toggle = page.locator("#lookahead-toggle");
    await openFilters(page);
    await expect(page.locator("#lookahead-window"), "the date and weeks show only while the window is on").toBeHidden();
    await toggle.click();
    await expect(page.locator("#lookahead-window")).toBeVisible();
    for (const [input, value] of [["#lookahead-start", day], ["#lookahead-weeks", "3"]]) {
      await openFilters(page);
      await page.locator(input).fill(value);
      await page.locator(input).blur();
    }
    await g.nextFrame(3);
    await expect(toggle).toHaveAttribute("aria-pressed", "true");
    const s = await g.status();
    expect(s.lookahead).toEqual({ on: true, start: day, weeks: 3 });
    expect(s.rowCount).toBeGreaterThan(0);
    expect(s.rowCount).toBeLessThan(n);
    await expect(count).toHaveText(`Lookahead · ${fmt.format(s.rowCount)} of ${fmt.format(n)} rows`);
    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-pressed", "false");
    await rowCountIs(page, n);
    expect((await g.status()).lookahead.on).toBe(false);
    await openFilters(page);
    await expect(page.locator("#lookahead-window")).toBeHidden();
  });

  await test.step("5. Ctrl+F finds activities by name in outline order; Enter and Shift+Enter step, Escape clears", async () => {
    // The query is a summary's outline path, so its matches are the summary and its subtree, read here in outline order.
    const rows = await allRows(page);
    const { query, hits } = await page.evaluate((all) => {
      const names = all.map(([task]) => window.gantt!.app.task_name(task).toLowerCase());
      for (const [i, row] of all.entries()) {
        const hits = all.filter((_, at) => names[at]!.includes(names[i]!)).map(([task]) => task);
        if (row[5] !== 0 && hits.length >= 3 && hits.length <= 200) return { query: names[i]!.toUpperCase(), hits };
      }
      return { query: "", hits: [] as number[] };
    }, rows);
    expect(hits.length, `a name with 3 to 200 matches (${JSON.stringify(query)})`).toBeGreaterThanOrEqual(3);
    const findCount = page.locator("#find-count");
    await page.keyboard.press("Control+f");
    await expect(page.locator("#find-input")).toBeFocused();
    await page.keyboard.press("1");
    await g.nextFrame(3);
    expect((await g.status()).rowCount, "a level key typed in the find box collapses nothing").toBe(n);
    await page.locator("#find-input").fill(query);
    await selectedIs(page, hits[0]!);
    await expect(findCount).toHaveText(`1 of ${fmt.format(hits.length)}`);
    await page.keyboard.press("Enter");
    await selectedIs(page, hits[1]!);
    await page.keyboard.press("Enter");
    await selectedIs(page, hits[2]!);
    await expect(findCount).toHaveText(`3 of ${fmt.format(hits.length)}`);
    await page.keyboard.press("Shift+Enter");
    await selectedIs(page, hits[1]!);
    await expect(findCount).toHaveText(`2 of ${fmt.format(hits.length)}`);
    await expect(page.locator('#table-rows .row[data-selected="true"]')).toHaveAttribute("data-task", String(hits[1]));
    await page.locator("#find-input").fill("zzzz no such task");
    await expect(findCount).toHaveText("no matches");
    await page.keyboard.press("Enter");
    expect((await g.status()).selected?.task, "Enter with no match keeps the selection").toBe(hits[1]);
    await page.keyboard.press("Escape");
    await expect(page.locator("#find-input")).toHaveValue("");
  });

  await test.step("6. clicks a critical bar and sees its driving path", async () => {
    const bar = await page.evaluate(() => {
      const app = window.gantt!.app;
      const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as Row[];
      const critical = rows.flatMap((row, at) => (row[5] === 0 && row[4] && row[1] < row[2] ? [at] : []));
      const at = critical[Math.floor(critical.length / 2)]!;
      return { row: at, task: rows[at]![0], es: app.row_start(at)! };
    });
    await page.evaluate((y) => (document.getElementById("table-body")!.scrollTop = y), Math.max(0, (bar.row - 5) * ROW));
    await page.mouse.move(box.x + box.width / 2, box.y + AXIS + 10 * ROW);
    for (let i = 0, last = Infinity; i < 200; i++) {
      await g.nextFrame();
      const dx = (await page.evaluate((t) => window.gantt!.app.x_of(t), bar.es)) - box.width / 5;
      if (Math.abs(dx) < 1 || Math.abs(dx) >= last) break;
      last = Math.abs(dx);
      await page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 2000), 0);
    }
    await g.nextFrame(2);
    const { scrollY } = await g.status();
    const x = box.x + (await page.evaluate((t) => window.gantt!.app.x_of(t), bar.es)) + 1;
    await page.mouse.click(x, box.y + AXIS + bar.row * ROW - scrollY + ROW / 2);
    await selectedIs(page, bar.task);
    const selected = (await g.status()).selected!;
    expect(selected.backCount, `critical task ${bar.task} has a driving path back`).toBeGreaterThan(0);
    await expect(page.locator('#table-rows .row[data-selected="true"]')).toHaveAttribute("data-row", String(bar.row));
  });
});
