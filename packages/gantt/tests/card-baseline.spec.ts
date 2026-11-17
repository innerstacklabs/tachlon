import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// The hover card's baseline lines (gs#703, row 4): a task with a baseline entry gets Baseline start and Baseline finish and their
// variance in working days; a summary, a task with no entry and a session with no baseline get none. The spec reads the card's DOM.

const labels = (page: Page): Promise<string[]> =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#hover-card .details > :nth-child(odd)")].map((el) => el.textContent ?? ""));

const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate(
    (name) => {
      const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
      const at = cells.findIndex((el) => el.textContent === name);
      return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
    },
    label,
  );

/** The first drawn leaf row with a duration of whole days: its row, task and days. */
const firstLeaf = (page: Page): Promise<{ row: number; task: number; days: number }> =>
  page.evaluate(() => {
    for (const el of document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")) {
      const task = Number(el.dataset.task);
      const days = /^(\d+)d$/.exec(window.gantt!.app.task_duration(task));
      if (days !== null && Number(days[1]) > 0) return { row: Number(el.dataset.row), task, days: Number(days[1]) };
    }
    throw new Error("no leaf row with a duration in days");
  });

test("a task with a baseline entry shows its baseline dates and variance on the card; with no baseline the card is as before", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  const { row, task, days } = await firstLeaf(page);
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;

  /** Move the pointer along the row until the card is for `task`. */
  const hoverTask = async (): Promise<void> => {
    await page.mouse.move(canvas.x + 1, y);
    for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
      await page.mouse.move(x, y);
      await g.nextFrame(1);
      if ((await g.status()).hover?.task === task) return;
    }
    throw new Error("no bar of the task under the pointer");
  };

  await hoverTask();
  await expect(page.locator("#hover-card")).toBeVisible();
  const before = await labels(page);
  expect(before).not.toContain("Baseline start");
  expect(before).not.toContain("Baseline finish");

  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await page.evaluate(([t, d]) => window.gantt!.app.set_duration(t!, `${d}d`), [task, days + 5] as const);
  await g.nextFrame(3);
  await hoverTask();
  await expect(page.locator("#hover-card")).toBeVisible();
  const after = await labels(page);
  expect(after, "the lines it had, and the two new ones").toEqual(expect.arrayContaining([...before, "Baseline start", "Baseline finish"]));
  expect(await valueOf(page, "Baseline start")).toMatch(/^\d{4}-\d\d-\d\d \d\d:\d\d \(0\)$/);
  expect(await valueOf(page, "Baseline finish")).toMatch(/^\d{4}-\d\d-\d\d \d\d:\d\d \(\+5d\)$/);

  await page.locator("#baseline-clear").click();
  await g.nextFrame(3);
  await hoverTask();
  expect(await labels(page), "no baseline, no extra lines").not.toContain("Baseline start");
});

test("a summary's card shows no baseline lines", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  const found = await page.evaluate(() => {
    const el = document.querySelector<HTMLElement>("#table-rows .row:not([data-kind='0'])");
    return el === null ? null : Number(el.dataset.row);
  });
  expect(found, "a summary row").not.toBeNull();
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + found! * s.rowHeight + s.rowHeight / 2;
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.summary === true) break;
  }
  expect((await g.status()).hover?.summary, "a summary bar under the pointer").toBe(true);
  await expect(page.locator("#hover-card")).toBeVisible();
  const shown = await labels(page);
  expect(shown).toContain("Start");
  expect(shown).not.toContain("Baseline start");
  expect(shown).not.toContain("Baseline finish");
});
