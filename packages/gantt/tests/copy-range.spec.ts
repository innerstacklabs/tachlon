import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Copy (gs#703, R5): Ctrl+C or Cmd+C with focus in the chart puts the range (or the selected row) on the clipboard as a
// header line and one tab-separated line per row. The spec dispatches a `copy` ClipboardEvent carrying a DataTransfer
// and reads what the page set as `text/plain`, so it needs no clipboard permission.

const SIZE = 2000;

const cell = (g: Gantt, row: number, column = "dur") => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

/** Fire a `copy` on the focused element and return what the page put on the clipboard and whether it took the event. */
const copy = (page: Page): Promise<{ text: string; taken: boolean }> =>
  page.evaluate(() => {
    const data = new DataTransfer();
    const event = new ClipboardEvent("copy", { clipboardData: data, bubbles: true, cancelable: true });
    (document.activeElement ?? document.body).dispatchEvent(event);
    return { text: data.getData("text/plain"), taken: event.defaultPrevented };
  });

test("copying a three-row range gives a header line and three tab-separated lines", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 3).click();
  await g.page.keyboard.press("Shift+ArrowDown");
  await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  expect((await g.status()).range).toEqual([3, 5, 5]);

  const { text, taken } = await copy(g.page);

  expect(taken).toBe(true);
  const lines = text.split("\n").map((line) => line.split("\t"));
  expect(lines[0]).toEqual(["Task", "Dur", "Start", "Finish", "Float"]);
  expect(lines).toHaveLength(4);
  for (const [i, row] of [3, 4, 5].entries()) {
    expect(lines[i + 1][1], "Dur reads as the table shows it").toBe((await cell(g, row, "dur").innerText()).trim());
    expect(lines[i + 1][2], "Start reads as the table shows it").toBe((await cell(g, row, "start").innerText()).trim());
  }
});

test("with no range the selected row copies", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 4).click();
  await g.nextFrame(2);
  expect((await g.status()).range).toBeNull();

  const { text } = await copy(g.page);

  expect(text.split("\n")).toHaveLength(2);
});
