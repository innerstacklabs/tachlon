import { expect, test } from "./support/gantt";
import type { PrintPages } from "../src/api";

// The on-screen legend strip (gs#703): the print legend's labels, swatches in the theme tokens. That it is not drawn in print is checked in journey-j5.spec.ts.

const TOKENS = ["bar", "crit", "crit", "text", "text", "sum", "link", "today", "nonwork", "sel-edge", "sel-edge"];

test("the legend names the printed legend's entries and its swatches take the theme tokens in light and dark", async ({ g }) => {
  await g.open({ n: 50 });
  const printed = await g.page.evaluate(() => {
    const pages = window.gantt!.app.print_pages("t", "") as PrintPages;
    const doc = new DOMParser().parseFromString(pages[0]!, "image/svg+xml");
    const labels = [...doc.querySelectorAll("rect.legend-swatch")].map((rect) => rect.nextElementSibling?.textContent ?? "");
    const known = JSON.parse((window.gantt!.app.constructor as unknown as { theme_tokens(): string }).theme_tokens()) as string[];
    return { labels, known };
  });
  expect(printed.labels).toHaveLength(TOKENS.length);
  expect(printed.labels.slice(-2)).toEqual(["Proposal ghost", "Just-moved outline"]);
  for (const token of TOKENS) expect(printed.known).toContain(token);

  const legend = g.page.locator("ul#legend");
  await expect(legend).toHaveAttribute("aria-label", "Legend");
  expect(await g.page.evaluate(() => document.querySelector(".gantt")!.lastElementChild!.id)).toBe("legend");
  await expect(legend.locator("li")).toHaveText(printed.labels);

  for (const theme of ["light", "dark"]) {
    await g.page.evaluate((t) => document.querySelector(".gantt")!.setAttribute("data-theme", t), theme);
    const colours = await g.page.evaluate((tokens) => {
      const root = document.querySelector(".gantt")!;
      const swatches = [...document.querySelectorAll("#legend .legend-swatch")];
      return swatches.map((swatch, i) => {
        const probe = document.createElement("i");
        probe.style.background = `var(--gantt-${tokens[i]})`;
        root.append(probe);
        const want = getComputedStyle(probe).backgroundColor;
        probe.remove();
        return { have: getComputedStyle(swatch).backgroundColor, want };
      });
    }, TOKENS);
    expect(colours).toHaveLength(TOKENS.length);
    for (const c of colours) expect(c.have, theme).toBe(c.want);
  }
  // The two themes differ, so the swatches really follow the attribute.
  const dark = await g.page.evaluate(() => getComputedStyle(document.querySelector("#legend .legend-swatch")!).backgroundColor);
  await g.page.evaluate(() => document.querySelector(".gantt")!.setAttribute("data-theme", "light"));
  const light = await g.page.evaluate(() => getComputedStyle(document.querySelector("#legend .legend-swatch")!).backgroundColor);
  expect(dark).not.toBe(light);
});
