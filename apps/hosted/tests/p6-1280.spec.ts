import { expect, test } from "@playwright/test";
import { readSamplesJson } from "./samples-file";
import { waitReady, waitSampleSettled } from "./support/page";

// A P6 file at 1280 px (gs#866): the toolbar's file line, data date and Find placeholder show in full, and the task list leaves the
// chart at least half the width. packages/gantt/tests/p6-1280.spec.ts holds the ledger sample; these are the medium and large samples of the menu.
test.use({ viewport: { width: 1280, height: 800 } });

const SAMPLES = readSamplesJson<{ samples: { id: string; format: string }[] }>().samples.filter((sample) => ["medium-5k", "large-50k"].includes(sample.id));

for (const sample of SAMPLES) {
  test(`${sample.id} at 1280 px: the toolbar text is whole and the chart keeps half the width`, async ({ page }) => {
    await page.goto(`/?sample=${sample.id}`);
    await waitReady(page, { frames: 10, timeout: 180_000 });
    await waitSampleSettled(page, 120_000);
    await page.waitForFunction(() => window.gantt!.status().frames > 0);
    const seen = await page.evaluate(() => {
      const context = document.createElement("canvas").getContext("2d")!;
      const byId = (id: string) => document.getElementById(id)!;
      const find = byId("find-input") as HTMLInputElement;
      const style = getComputedStyle(find);
      context.font = `${style.fontSize} ${style.fontFamily}`;
      const pad = parseFloat(style.paddingLeft) + parseFloat(style.paddingRight) + parseFloat(style.borderLeftWidth) + parseFloat(style.borderRightWidth);
      const cut = (el: HTMLElement): number => el.scrollWidth - el.clientWidth;
      return {
        overflow: cut(byId("toolbar")),
        file: cut(byId("open-file")),
        date: cut(byId("data-date")),
        dateText: byId("data-date").textContent,
        find: context.measureText(find.placeholder).width + pad - find.getBoundingClientRect().width,
        chart: byId("timeline").getBoundingClientRect().width / window.innerWidth,
        progress: byId("table").dataset["progress"],
      };
    });
    expect(seen.progress, "a P6 file shows the progress columns").toBe("true");
    expect(seen.overflow, "the toolbar overflows by").toBeLessThanOrEqual(0);
    expect(seen.file, "the file line is cut by").toBeLessThanOrEqual(0);
    expect(seen.date, `the data date "${seen.dateText}" is cut by`).toBeLessThanOrEqual(0);
    expect(seen.find, "the Find placeholder is cut by").toBeLessThanOrEqual(0.5);
    expect(seen.chart, "the chart's share of the width").toBeGreaterThanOrEqual(0.5);
  });
}
