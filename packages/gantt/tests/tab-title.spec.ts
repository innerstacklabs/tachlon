// The tab names the open file, marks unsaved changes and asks before leaving (gs#703).
import { readFileSync } from "node:fs";
import { PRODUCT_NAME } from "../src/product";
import { expect, test } from "./support/gantt";

const FIXTURE = {
  name: "one-project.xer",
  mimeType: "application/octet-stream",
  buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)),
};

test("the_tab_names_the_file_marks_unsaved_changes_and_asks_before_leaving", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  // A generated demo has no file: the title is not touched, and a leave is not asked about.
  expect(await page.title()).not.toContain(` - ${PRODUCT_NAME}`);
  const leave = (): Promise<boolean> =>
    page.evaluate(() => {
      const event = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(event);
      return event.defaultPrevented;
    });
  expect(await leave()).toBe(false);

  await page.setInputFiles("#open-input", FIXTURE);
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);
  await expect.poll(() => page.title()).toBe(`one-project.xer - ${PRODUCT_NAME}`);
  expect(await leave()).toBe(false);

  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await g.nextFrame(2);
  await expect.poll(() => page.title()).toBe(`• one-project.xer - ${PRODUCT_NAME}`);
  expect(await leave()).toBe(true);

  await page.locator("#app").focus();
  const download = page.waitForEvent("download");
  await page.keyboard.press("ControlOrMeta+S");
  await download;
  await g.nextFrame(2);
  await expect.poll(() => page.title()).toBe(`one-project.xer - ${PRODUCT_NAME}`);
  expect(await leave()).toBe(false);
});
