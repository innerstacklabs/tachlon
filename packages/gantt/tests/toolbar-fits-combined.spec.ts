import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// The toolbar in the combined filter state (gs#808): an activity-code filter with a long type and value label, a baseline set
// and Lookahead on, all at once, at 1280 px and at 1024 px. gs#794 made the toolbar fit in each single state; this is the state #806's review
// estimated from font metrics alone. The measurement is the one of toolbar-fits-filters.spec.ts. Set SHOTS to a directory to
// also write the light and dark screenshots of the state there.
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const LONG_TYPE = "Construction phase";
const LONG_VALUE = "Mechanical commissioning";
const LONG = Buffer.from(
  LEDGER.replace(
    /%E\s*$/,
    "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n" +
      `%R\t7000\t${LONG_TYPE}\n%R\t7001\tArea\n` +
      "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n" +
      `%R\t7100\t7000\tCivil\n%R\t7105\t7000\t${LONG_VALUE}\n%R\t7200\t7001\tNorth\n%R\t7201\t7001\tSouth\n%E\n`,
  ),
);

/** Where the toolbar's width, the file line and the find box stand now. */
const fit = (page: Page) =>
  page.evaluate(() => {
    const toolbar = document.getElementById("toolbar")!;
    const bar = toolbar.getBoundingClientRect();
    const inside = (id: string): boolean => {
      const box = document.getElementById(id)!.getBoundingClientRect();
      return box.left >= bar.left && box.right <= bar.right && box.top >= bar.top && box.bottom <= bar.bottom;
    };
    const visible = ["code-type", "code-value", "code-clear", "baseline-clear", "slipped-toggle", "lookahead-start", "lookahead-weeks"].filter(
      (id) => document.getElementById(id)!.getBoundingClientRect().width > 0,
    );
    return {
      scrollWidth: toolbar.scrollWidth,
      clientWidth: toolbar.clientWidth,
      file: inside("open-info"),
      find: inside("find-input"),
      columns: inside("columns-button"),
      visible,
      widths: Object.fromEntries([...toolbar.children].map((el) => [el.id || el.className, Math.round(el.getBoundingClientRect().width)])),
    };
  });

/** Press a control by its click handler: while the toolbar overflows a control can lie past the window's edge. */
const press = (page: Page, selector: string): Promise<void> => page.locator(selector).evaluate((el) => (el as HTMLElement).click());

/** Open the Filters menu if the page has one (the filters sit in it), run `act`, and close it again. */
async function inFilters(page: Page, act: () => Promise<void>): Promise<void> {
  const button = page.locator("#filters-button");
  const menu = (await button.count()) > 0;
  if (menu) await button.click();
  await act();
  // Choosing a code value hands focus back to the chart and closes the menu; Escape on the chart would clear its selection.
  if (menu && (await page.locator("#filters-panel").isVisible())) {
    await page.keyboard.press("Escape");
    await expect(page.locator("#filters-panel")).toBeHidden();
  }
}

// `host`: the chart is moved into a box that wide inside a 1280 px window (an embedded chart); the width that counts is the box's.
const CASES = [{ width: 1280 }, { width: 1024 }, { width: 800, host: true }];
for (const { width, host } of CASES) {
  test(`the toolbar fits at ${width} px${host ? " (an embedded chart in a 1280 px window)" : ""} with a long code filter, a baseline and Lookahead together`, async ({ g }, info) => {
    await g.page.setViewportSize({ width: host ? 1280 : width, height: 800 });
    await g.open({ n: 300 });
    await g.page.setInputFiles("#open-input", { name: "coded.xer", mimeType: "application/octet-stream", buffer: LONG });
    await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await g.nextFrame(3);

    await press(g.page, "#baseline-set");
    await g.nextFrame(2);
    await inFilters(g.page, async () => {
      await expect(g.page.locator("#code-filter")).toBeVisible();
      await press(g.page, "#lookahead-toggle");
      await g.page.selectOption("#code-type", LONG_TYPE);
      await g.page.selectOption("#code-value", LONG_VALUE);
    });
    await g.page.waitForFunction(() => window.gantt!.status().filter === "code");
    await g.nextFrame(3);
    if (host) {
      await g.page.evaluate((w) => {
        const box = document.createElement("div");
        box.style.cssText = `position: absolute; left: 0; top: 0; width: ${w}px; height: 700px`;
        document.body.append(box);
        box.append(document.getElementById("app")!);
      }, width);
      await g.nextFrame(3);
    }

    const seen = await fit(g.page);
    info.annotations.push({ type: "measured", description: JSON.stringify(seen) });
    console.log(`combined state at ${width} px: ${JSON.stringify(seen)}`);

    const dir = process.env.SHOTS;
    if (dir && width === 1280) {
      // The import notice covers the strip's right half; close it for the picture.
      await g.page.locator("#unsupported-close").click();
      for (const scheme of ["light", "dark"] as const) {
        await g.page.emulateMedia({ colorScheme: scheme });
        await g.nextFrame(3);
        await g.page.screenshot({ path: `${dir}/toolbar-combined-1280-${scheme}-closed.png`, clip: { x: 0, y: 0, width: 1280, height: 120 } });
        if ((await g.page.locator("#filters-button").count()) > 0) {
          await g.page.locator("#filters-button").click();
          await g.nextFrame(2);
          await g.page.screenshot({ path: `${dir}/toolbar-combined-1280-${scheme}-open.png`, clip: { x: 0, y: 0, width: 1280, height: 320 } });
          await g.page.keyboard.press("Escape");
        }
      }
    }

    expect.soft(seen.scrollWidth, "the toolbar's scrollWidth").toBeLessThanOrEqual(seen.clientWidth);
    expect.soft(seen.file, "the file line is inside the toolbar").toBe(true);
    expect.soft(seen.find, "the find box is inside the toolbar").toBe(true);
    expect.soft(seen.columns, "the Columns button is inside the toolbar").toBe(true);
  });
}
