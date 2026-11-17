import type { Page } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product";
import { expect, test, type Gantt, openFilters } from "./support/gantt";

// The Ledger toolbar strip and the status line (gs#110 PR 3). Each spec drives one control through the page and reads
// the result from the chart (window.gantt.status), so removing the control fails its spec at the first locator.

const SIZE = 2000;
const number = new Intl.NumberFormat("en-US");

/** The first leaf task's full name, and how many tasks a search for it finds. */
async function aQuery(g: Gantt): Promise<{ query: string; matches: number }> {
  return g.page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, 60)) as [number, string, string, number | null, boolean, number][];
    const leaf = rows.find((row) => row[5] === 0);
    if (!leaf) throw new Error("no leaf row in the first 60");
    const query = app.task_name(leaf[0]);
    const matches = app.find(query);
    app.find("");
    return { query, matches };
  });
}

/** Where keyboard focus is: an element id or tag name. */
function focused(page: Page): Promise<string | undefined> {
  return page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
}

test("find from the toolbar selects the match, counts it, steps with Enter and clears with Escape", async ({ g }) => {
  await g.open({ n: SIZE });
  const { query, matches } = await aQuery(g);
  expect(matches, "the query finds at least the task it came from").toBeGreaterThanOrEqual(1);

  const input = g.page.locator("#toolbar #find-input");
  await expect(input, "the find box is part of the toolbar and always visible").toBeVisible();
  await input.fill(query);
  await expect(g.page.locator("#find-count")).toHaveText(`1 of ${number.format(matches)}`);
  await g.nextFrame(3);
  expect((await g.status()).selected, "the first match is selected").not.toBeNull();

  if (matches > 1) {
    await input.press("Enter");
    await expect(g.page.locator("#find-count")).toHaveText(`2 of ${number.format(matches)}`);
  }

  await input.press("Escape");
  await expect(input).toHaveValue("");
  await expect(g.page.locator("#find-count")).toHaveText("");
  expect(await focused(g.page), "Escape gives focus back to the chart").toBe("app");
});

test("Ctrl+F focuses the toolbar's find box, and only while focus is inside the chart", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.keyboard.press("Control+f");
  expect(await focused(g.page)).toBe("find-input");
  await g.page.keyboard.press("Escape");

  // With focus outside the chart (the page body), the key is the browser's own.
  await g.page.evaluate(() => (document.activeElement as HTMLElement).blur());
  expect(await focused(g.page)).toBe("BODY");
  await g.page.keyboard.press("Control+f");
  expect(await focused(g.page), "Ctrl+F outside the chart leaves focus alone").toBe("BODY");
});

test("Critical only filters the rows, shows pressed, and a second click restores them", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = await g.status();
  const toggle = g.page.locator("#toolbar #critical-toggle");
  await expect(toggle).toHaveAttribute("aria-pressed", "false");

  await openFilters(g.page);
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await g.nextFrame(3);
  const filtered = await g.status();
  expect(filtered.filter).toBe("critical");
  expect(filtered.rowCount, "fewer rows with the filter on").toBeLessThan(all.rowCount);
  expect(filtered.rowCount).toBeGreaterThan(0);

  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await g.nextFrame(3);
  const restored = await g.status();
  expect(restored.filter).toBe("off");
  expect(restored.rowCount).toBe(all.rowCount);
});

test("Collapse to level 1 and 2 hide rows and All restores them", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = (await g.status()).rowCount;

  await g.page.locator("#toolbar #level-1").click();
  await g.nextFrame(3);
  const one = (await g.status()).rowCount;
  expect(one, "level 1 shows only the top-level rows").toBeLessThan(all);

  await g.page.locator("#toolbar #level-2").click();
  await g.nextFrame(3);
  const two = (await g.status()).rowCount;
  expect(two, "level 2 shows more than level 1 and fewer than all").toBeGreaterThan(one);
  expect(two).toBeLessThan(all);

  await g.page.locator("#toolbar #level-all").click();
  await g.nextFrame(3);
  expect((await g.status()).rowCount).toBe(all);
});

test("the row count follows the filter", async ({ g }) => {
  await g.open({ n: SIZE });
  const count = g.page.locator("#toolbar #row-count");
  const all = await g.status();
  await expect(count).toHaveText(`${number.format(all.rowCount)} rows`);

  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await g.nextFrame(3);
  const filtered = await g.status();
  await expect(count).toHaveText(
    `Critical only · ${number.format(filtered.keptLeaves!)} of ${number.format(filtered.n)} activities`,
  );

  await g.page.locator("#level-1").click();
  await g.nextFrame(3);
  const collapsed = await g.status();
  await expect(count).toHaveText(
    `Critical only · ${number.format(collapsed.keptLeaves!)} of ${number.format(collapsed.n)} activities`,
  );
});

test("the zoom buttons scale the view about its middle by about 2x", async ({ g }) => {
  await g.open({ n: SIZE });
  const before = (await g.status()).pxPerMinute;
  const middle = await g.page.evaluate(() => {
    const canvas = document.getElementById("timeline")!;
    return window.gantt!.app.time_at(canvas.clientWidth / 2);
  });

  await g.page.locator("#toolbar #zoom-in").click();
  await g.nextFrame(2);
  const zoomedIn = (await g.status()).pxPerMinute;
  expect(zoomedIn / before).toBeGreaterThan(1.9);
  expect(zoomedIn / before).toBeLessThan(2.1);
  const middleAfter = await g.page.evaluate(() => {
    const canvas = document.getElementById("timeline")!;
    return window.gantt!.app.time_at(canvas.clientWidth / 2);
  });
  expect(Math.abs(middleAfter - middle) * zoomedIn, "the instant at the middle moves under 1 px").toBeLessThan(1);

  await g.page.locator("#toolbar #zoom-out").click();
  await g.nextFrame(2);
  expect((await g.status()).pxPerMinute / before).toBeGreaterThan(0.95);
  expect((await g.status()).pxPerMinute / before).toBeLessThan(1.05);
});

test("the status line shows and hides with the s key, only while focus is inside the chart", async ({ g }) => {
  await g.open({ n: SIZE });
  const line = g.page.locator("#app #status-line");
  await expect(line, "hidden until asked for").toBeHidden();
  expect(await g.page.locator("#overlay").textContent(), "the live numbers are not in the message line").toBe("");

  await g.page.keyboard.press("s");
  await expect(line).toBeVisible();
  await g.nextFrame(3);
  await expect(line).toContainText(`${number.format(SIZE)} tasks`);
  await expect(line).toContainText("frame");
  const height = (await line.boundingBox())!.height;
  expect(height, "one fixed-height line").toBe(22);

  await g.page.keyboard.press("s");
  await expect(line).toBeHidden();

  // Outside the chart the key does nothing: neither a real key press nor an event sent to the document.
  await g.page.evaluate(() => (document.activeElement as HTMLElement).blur());
  await g.page.keyboard.press("s");
  await expect(line).toBeHidden();
  await g.page.evaluate(() => {
    for (const target of [document.body, document.documentElement, document, window] as EventTarget[]) {
      target.dispatchEvent(new KeyboardEvent("keydown", { key: "s", bubbles: true }));
      target.dispatchEvent(new KeyboardEvent("keydown", { key: "1", bubbles: true }));
    }
  });
  await g.nextFrame(2);
  await expect(line).toBeHidden();
  expect((await g.status()).rowCount, "1 outside the chart collapses nothing").toBe((await g.status()).n);
});

test("clicking the chart gives it focus, so its keys work after a toolbar click", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.locator("#find-input").click();
  expect(await focused(g.page)).toBe("find-input");
  const box = await g.box();
  await g.page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  expect(await focused(g.page)).toBe("app");
  await g.page.keyboard.press("1");
  await g.nextFrame(3);
  const s = await g.status();
  expect(s.rowCount).toBeLessThan(s.n);
});

test("the file group holds the Open button, and its file line is empty until a file is opened", async ({ g }) => {
  await g.open({ n: SIZE });
  const group = g.page.locator("#toolbar #toolbar-file");
  await expect(group).toBeVisible();
  await expect(group.locator("#open-button")).toHaveText("Open…");
  await expect(group.locator("#open-button")).toHaveAttribute("title", `Open a schedule: a P6 XER, an MS Project XML or a ${PRODUCT_NAME} file (Ctrl+O), or drop one on the chart.`);
  await expect(group.locator("#open-input")).toHaveAttribute("accept", ".xer,.xml,.gantt");
  await expect(group.locator("#open-info")).toHaveText("");
});

test("the chart works with the toolbar and the status line removed", async ({ g }) => {
  // Remove the app's chrome the moment the parser inserts it, before the module script runs.
  await g.page.addInitScript(() => {
    const strip = (): void => {
      document.getElementById("toolbar")?.remove();
      document.getElementById("status-line")?.remove();
    };
    new MutationObserver(strip).observe(document, { childList: true, subtree: true });
  });
  await g.open({ n: SIZE });
  expect(await g.page.locator("#toolbar").count()).toBe(0);
  expect(await g.page.locator("#status-line").count()).toBe(0);

  const s0 = await g.status();
  expect(s0.frames).toBeGreaterThan(0);
  const canvas = await g.box();
  expect(canvas.y, "the chart takes the toolbar's room back").toBe(0);
  expect(canvas.height).toBeGreaterThan(700);

  // Keys, filters and zoom are the chart's own commands and still run.
  await g.page.keyboard.press("1");
  await g.nextFrame(3);
  const collapsed = await g.status();
  expect(collapsed.rowCount).toBeLessThan(s0.rowCount);
  await g.page.keyboard.press("0");
  await g.page.keyboard.press("Shift+C");
  await g.nextFrame(3);
  expect((await g.status()).filter).toBe("critical");
  await g.page.keyboard.press("s");
  await g.page.keyboard.press("Control+f");
  await g.nextFrame(2);
  expect((await g.status()).frames, "the chart keeps drawing").toBeGreaterThan(s0.frames);
});
