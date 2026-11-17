import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Select all (gs#703): Ctrl+A or Cmd+A with focus in the chart makes the visible rows a range, and Ctrl+C then copies a header
// line and one line per row. In the find box the key selects the text as before.

const SIZE = 2000;

const cell = (g: Gantt, row: number, column = "dur") => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

/** Fire a `copy` on the focused element and return what the page put on the clipboard. */
const copy = (page: Page): Promise<{ text: string; taken: boolean }> =>
  page.evaluate(() => {
    const data = new DataTransfer();
    const event = new ClipboardEvent("copy", { clipboardData: data, bubbles: true, cancelable: true });
    (document.activeElement ?? document.body).dispatchEvent(event);
    return { text: data.getData("text/plain"), taken: event.defaultPrevented };
  });

test("Ctrl+A makes every visible row a range and a copy gives a line per row", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 3).click();
  await g.page.keyboard.press("Control+a");
  await g.nextFrame(2);

  const { rowCount, range } = await g.status();
  expect(range).toEqual([0, rowCount - 1, rowCount - 1]);

  const { text, taken } = await copy(g.page);
  expect(taken).toBe(true);
  expect(text.split("\n")).toHaveLength(rowCount + 1);
});

test("with Critical only on, Ctrl+A copies the critical path", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = (await g.status()).rowCount;
  await cell(g, 2).click();
  expect(await g.page.evaluate(() => window.gantt!.app.set_critical_only(true))).toBe(true);
  await g.nextFrame(2);
  const shown = (await g.status()).rowCount;
  expect(shown, "the filter hides rows").toBeLessThan(all);

  await g.page.keyboard.press("Control+a");
  await g.nextFrame(2);

  expect((await g.status()).range).toEqual([0, shown - 1, shown - 1]);
  expect((await copy(g.page)).text.split("\n")).toHaveLength(shown + 1);
});

test("in the find box Ctrl+A selects the text and makes no range", async ({ g }) => {
  await g.open({ n: SIZE });
  const find = g.page.locator("#find-input");
  await find.click();
  await g.page.keyboard.type("task");
  await g.page.keyboard.press("Control+a");
  await g.nextFrame(2);

  expect((await g.status()).range).toBeNull();
  expect(await find.evaluate((input: HTMLInputElement) => [input.selectionStart, input.selectionEnd])).toEqual([0, 4]);
});
