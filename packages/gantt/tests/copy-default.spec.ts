import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Copy keeps the browser's own behaviour (gs#703): with a row selected, a `copy` in a text box, over selected page text,
// with nothing selected, or outside the chart root is left to the browser. The chart takes the event only when it is the
// target inside the root with no text selected and a row or range to copy.

const SIZE = 2000;

const cell = (g: Gantt, row: number, column = "dur") => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

/** Fire a `copy` on the given element (the focused one by default) and return what the page put on the clipboard and whether it took the event. */
const copy = (page: Page, target?: string): Promise<{ text: string; taken: boolean }> =>
  page.evaluate((selector) => {
    const data = new DataTransfer();
    const event = new ClipboardEvent("copy", { clipboardData: data, bubbles: true, cancelable: true });
    (selector ? document.querySelector(selector)! : (document.activeElement ?? document.body)).dispatchEvent(event);
    return { text: data.getData("text/plain"), taken: event.defaultPrevented };
  }, target);

const expectLeftToBrowser = (r: { text: string; taken: boolean }) => {
  expect(r.taken).toBe(false);
  expect(r.text).toBe("");
};

test.describe("copy stays the browser's", () => {
  test.beforeEach(async ({ g }) => {
    await g.open({ n: SIZE });
    await cell(g, 4).click();
    await g.nextFrame(2);
  });

  test("control: with a row selected, a copy on the chart root is taken", async ({ g }) => {
    const { text, taken } = await copy(g.page, "#app");
    expect(taken).toBe(true);
    expect(text.split("\n")).toHaveLength(2);
  });

  test("in the find box holding typed text with the caret at the end", async ({ g }) => {
    await g.page.focus("#find-input");
    await g.page.keyboard.type("task");
    const sel = await g.page.evaluate(() => {
      const input = document.activeElement as HTMLInputElement;
      return { id: input.id, start: input.selectionStart, end: input.selectionEnd, length: input.value.length };
    });
    expect(sel).toEqual({ id: "find-input", start: 4, end: 4, length: 4 });
    expectLeftToBrowser(await copy(g.page));
  });

  test("over selected page text, on the chart root", async ({ g }) => {
    const selected = await g.page.evaluate(() => {
      getSelection()!.selectAllChildren(document.getElementById("row-count")!);
      return getSelection()!.toString();
    });
    expect(selected, "the row count has text to select").not.toBe("");
    expectLeftToBrowser(await copy(g.page, "#app"));
  });

  test("after Escape clears the selection, on the chart root", async ({ g }) => {
    expect((await g.status()).selected, "a row is selected first").not.toBeNull();
    await g.page.focus("#app");
    await g.page.keyboard.press("Escape");
    await g.nextFrame(2);
    expect((await g.status()).selected, "Escape cleared the selection").toBeNull();
    expectLeftToBrowser(await copy(g.page, "#app"));
  });

  test("on the document body, outside the chart root", async ({ g }) => {
    expectLeftToBrowser(await copy(g.page, "body"));
  });
});
