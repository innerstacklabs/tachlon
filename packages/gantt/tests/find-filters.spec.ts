import type { Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// Find follows a filter (gs#703): with a search open on its third match, turning a filter on through its control starts the
// search again at the first match of the filtered rows, whichever control it was: the Filters panel button or its key. The
// restart is `find.follow`, which `packages/gantt/src/index.ts` registers on the chart's `change` event (gs#1194); nothing else pins it.
// The pages start on WebGL2: a WebGPU device lost mid-run reloads the page on WebGL2 and the find position with it (gantt#1606), and
// `expectWebGL2` fails a read of the find count on a chart that is not on WebGL2 by the pin, a WebGPU fallback included.

test.use({ webgl: true });

type Filter = "near" | "moved" | "slipped";

/** Find state is read only on the pinned WebGL2 backend; a chart a device loss left on its fallback is not the one the search ran in. */
async function expectWebGL2(page: Page): Promise<void> {
  const { backend, fallback } = await page.evaluate(() => ({ backend: window.gantt!.status().backend, fallback: window.gantt!.fallback }));
  expect({ backend, fallback }, "find state is read on the pinned WebGL2 backend, not on a fallback").toEqual({ backend: "WebGL2", fallback: null });
}

/**
 * With the filter on, pick from the shown rows' names the needle with the fewest matches, at least three, so it also matches
 * at least three rows with the filter off (the shown rows are among them); then turn the filter off again and return the
 * needle with the task of its first match among the shown rows.
 */
async function pickNeedle(page: Page, filter: Filter): Promise<{ needle: string; first: number }> {
  const picked = await page.evaluate((which) => {
    const app = window.gantt!.app;
    const set = { near: app.set_near_critical, moved: app.set_moved_only, slipped: app.set_slipped_only }[which].bind(app);
    set(true);
    const rows = JSON.parse(app.table_rows(0, app.test_row_count())) as number[][];
    const names = rows.map((row) => app.task_name(row[0]).toLowerCase());
    set(false);
    const candidates = new Set<string>();
    for (const name of names) for (const word of name.split(/[^a-z0-9]+/)) if (word.length >= 3) candidates.add(word);
    let best: { needle: string; count: number; first: number } | null = null;
    for (const needle of candidates) {
      const hits = names.flatMap((name, i) => (name.includes(needle) ? [i] : []));
      if (hits.length >= 3 && (best === null || hits.length < best.count)) best = { needle, count: hits.length, first: rows[hits[0]][0] };
    }
    return best === null ? null : { needle: best.needle, first: best.first };
  }, filter);
  expect(picked, `a needle matches three rows under the ${filter} filter`).not.toBeNull();
  return picked!;
}

/** Wait for the find count to read `index of N` and return N from that same read (a second read could see a moved count: NaN). */
async function totalAt(page: Page, index: number): Promise<number> {
  let total = NaN;
  await expect.poll(async () => {
    const read = /^(\d+) of (\d+)$/.exec((await page.locator("#find-count").textContent()) ?? "");
    total = Number(read?.[2]);
    return Number(read?.[1]);
  }, `the find count reads ${index} of N`).toBe(index);
  return total;
}

/** Search for `needle`, press Enter twice (the second once the first shows), and expect the third match; return the total. */
async function searchToThird(page: Page, needle: string): Promise<number> {
  await page.locator("#find-input").fill(needle);
  await totalAt(page, 1);
  await page.keyboard.press("Enter");
  await totalAt(page, 2);
  await page.keyboard.press("Enter");
  const total = await totalAt(page, 3);
  await expectWebGL2(page);
  return total;
}

/** After the filter came on: the search restarted, so the count reads `1 of M` with fewer matches and the first match is selected. */
async function expectRestart(page: Page, before: number, first: number): Promise<void> {
  await expectWebGL2(page);
  const after = await totalAt(page, 1);
  expect(after, "the filter dropped some matches").toBeLessThanOrEqual(before);
  await expect.poll(async () => (await page.evaluate(() => window.gantt!.status())).selected?.task, "the first match is selected").toBe(first);
}

test("Near critical from the Filters panel restarts find at the first match", async ({ g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 2000 });
  const { needle, first } = await pickNeedle(g.page, "near");
  const before = await searchToThird(g.page, needle);
  await openFilters(g.page);
  await g.page.locator("#near-critical-toggle").click();
  expect((await g.status()).filter).toBe("near");
  await expectRestart(g.page, before, first);
});

test("Show moved from the panel and by Shift+M restarts find at the first match", async ({ g }) => {
  test.setTimeout(120_000);
  await openLedger(g);
  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  const { needle, first } = await pickNeedle(g.page, "moved");

  const before = await searchToThird(g.page, needle);
  await openFilters(g.page);
  await g.page.locator("#moved-filter").click();
  expect((await g.status()).filter).toBe("moved");
  await expectRestart(g.page, before, first);

  // Off again, search to the third match once more, then the key.
  await g.page.locator("#moved-filter").click();
  expect((await g.status()).filter).toBe("off");
  await g.nextFrame(2); // `change` fires from the frame loop; a find typed before it lands is restarted by it, so an Enter pressed in between is undone
  await g.page.keyboard.press("Escape");
  await g.page.locator("#find-input").fill("");
  const again = await searchToThird(g.page, needle);
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
  await g.nextFrame(2);
  await g.page.keyboard.press("Shift+M");
  await g.page.waitForFunction(() => window.gantt!.status().filter === "moved");
  await expectRestart(g.page, again, first);
});

test("Shift+S restarts find at the first match after a baseline and a slipping edit", async ({ g }) => {
  test.setTimeout(120_000);
  await openLedger(g);
  await g.page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await editDuration(g.page, "20d");
  await g.nextFrame(3);
  expect((await g.status()).baseline!.slipped, "the edit slipped some tasks").toBeGreaterThan(0);
  const { needle, first } = await pickNeedle(g.page, "slipped");

  const before = await searchToThird(g.page, needle);
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
  await g.nextFrame(2);
  await g.page.keyboard.press("Shift+S");
  await g.page.waitForFunction(() => window.gantt!.status().filter === "slipped");
  await expectRestart(g.page, before, first);
});
