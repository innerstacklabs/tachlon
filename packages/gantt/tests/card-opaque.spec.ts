// The hover card stands clear of the chart (gs#1037): its background is opaque in the light and the dark theme, so no bar shows through,
// and in dark it has a visible edge (a border or a shadow) drawn from the theme tokens. The spec hovers a task of the office fit-out and
// reads the card's computed style.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

/** Open the fit-out and hover the first drawn leaf row until the card shows. */
async function hoverFitout(g: Gantt): Promise<void> {
  const { page } = g;
  await g.open({ n: 50 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const row = await page.evaluate(() => Number(document.querySelector<HTMLElement>("#table-rows .row[data-kind='0']")!.dataset.row));
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover !== null && (await page.locator("#hover-card").isVisible())) return;
  }
  throw new Error("no bar under the pointer");
}

const style = (page: Page) =>
  page.evaluate(() => {
    const cs = getComputedStyle(document.getElementById("hover-card")!);
    return { bg: cs.backgroundColor, border: cs.borderTopWidth, shadow: cs.boxShadow };
  });

/** The alpha of a computed `rgb()` or `rgba()` colour. */
const alpha = (css: string): number => (/^rgba\(.*,\s*([\d.]+)\)$/.exec(css)?.[1] ? Number(/,\s*([\d.]+)\)$/.exec(css)![1]) : 1);

for (const scheme of ["light", "dark"] as const) {
  test(`the hover card is opaque in ${scheme}`, async ({ g, page }) => {
    test.setTimeout(120_000);
    await page.emulateMedia({ colorScheme: scheme });
    await hoverFitout(g);
    const s = await style(page);
    expect(alpha(s.bg), `background ${s.bg}`).toBe(1);
    if (scheme === "dark") expect(s.border !== "0px" || s.shadow !== "none", `border ${s.border}, shadow ${s.shadow}`).toBe(true);
  });
}
