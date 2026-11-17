import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// A filter's row count is the only sign that a filter narrows the chart (A devteam-1-368, on gs#1055): at 1280 px with a baseline set
// it stays visible and whole ("... N of M rows"), Find keeps the 90 px its hint needs, and the `?` button is a whole 18 px button.
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

test.use({ viewport: { width: 1280, height: 800 } });

test("with a baseline set and a filter on, the count 'N of M rows' is whole at 1280 px, Find keeps its 90 px hint and ? is whole", async ({ g }) => {
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: LEDGER });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.locator("#baseline-set").evaluate((el) => (el as HTMLElement).click());
  await expect(g.page.locator("#baseline-clear")).toBeVisible();
  await g.page.locator("#filters-button").click();
  await g.page.locator("#critical-toggle").click();
  await g.page.keyboard.press("Escape");
  await g.page.waitForFunction(() => window.gantt!.status().filter !== "off");
  await g.nextFrame(3);

  await expect(g.page.locator("#row-count")).toHaveText(/\d+ of \d+ activities$/);
  const seen = await g.page.evaluate(() => {
    const whole = (id: string): { needs: number; has: number } => {
      const el = document.getElementById(id)!;
      const range = document.createRange();
      range.selectNodeContents(el);
      return { needs: range.getBoundingClientRect().width, has: el.getBoundingClientRect().width };
    };
    const input = document.getElementById("find-input") as HTMLInputElement;
    const style = getComputedStyle(input);
    const canvas = document.createElement("canvas").getContext("2d")!;
    canvas.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    const room = input.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    const bar = document.getElementById("toolbar")!;
    return {
      count: whole("row-count"),
      file: whole("open-file"),
      date: whole("data-date"),
      hint: { needs: Math.ceil(canvas.measureText(input.placeholder).width), has: Math.floor(room) },
      keys: document.getElementById("keys-button")!.getBoundingClientRect().width,
      overflow: bar.scrollWidth - bar.clientWidth,
    };
  });
  console.log(`filtered count at 1280 px: ${JSON.stringify(seen)}`);
  expect(seen.count.has, "the count is shown").toBeGreaterThan(0);
  expect(seen.count.needs, "the count is whole").toBeLessThanOrEqual(seen.count.has + 0.01);
  expect(seen.hint.needs, "Find keeps its 90 px hint").toBeLessThanOrEqual(seen.hint.has);
  expect(seen.keys, "the ? button is whole").toBe(18);
  expect(seen.overflow, "the toolbar does not overflow").toBeLessThanOrEqual(0);
});
