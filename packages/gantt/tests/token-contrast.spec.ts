import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// The muted text (the row count, the status line) must read at 4.5:1 or more on the selected row and on the background,
// in the light set and in the dark set (WCAG AA for normal text).

const MIN = 4.5;

/** The root's computed colour tokens with `data-theme` forced, as `[r, g, b]` bytes. */
async function tokens(page: Page, theme: "light" | "dark"): Promise<Record<string, [number, number, number]>> {
  return page.evaluate((forced) => {
    const root = document.querySelector(".gantt") as HTMLElement;
    root.setAttribute("data-theme", forced);
    const probe = document.createElement("canvas").getContext("2d")!;
    const style = getComputedStyle(root);
    const read = (name: string): [number, number, number] => {
      probe.fillStyle = "#000000";
      probe.fillStyle = style.getPropertyValue(name).trim();
      probe.fillRect(0, 0, 1, 1);
      const d = probe.getImageData(0, 0, 1, 1).data;
      return [d[0], d[1], d[2]];
    };
    return { muted: read("--gantt-muted"), sel: read("--gantt-sel"), bg: read("--gantt-bg") };
  }, theme);
}

function luminance([r, g, b]: [number, number, number]): number {
  const lin = (c: number): number => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(a: [number, number, number], b: [number, number, number]): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

for (const theme of ["light", "dark"] as const) {
  test(`muted_text_reaches_4_5_to_1_on_selected_row_and_background_${theme}`, async ({ g, page }) => {
    await g.open({ n: 100, frames: 2 });
    const t = await tokens(page, theme);
    expect(contrast(t.muted, t.sel), `${theme}: muted on sel`).toBeGreaterThanOrEqual(MIN);
    expect(contrast(t.muted, t.bg), `${theme}: muted on bg`).toBeGreaterThanOrEqual(MIN);
  });
}
