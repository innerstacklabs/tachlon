import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// `data-theme` on the chart root forces the dark or the light token set whatever the system says (gs#703); no
// attribute follows the system, as before. The observer carries the change to the canvas, as it does a style edit.

const DARK_BG = "#0e1630";
const LIGHT_BG = "#f2f5f9";

/** The root's computed `--gantt-bg` and the canvas theme's `bg`, read after the observer's frame has run. */
async function read(page: Page): Promise<{ css: string; hook: string }> {
  return page.evaluate(
    () =>
      new Promise((done) => {
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            window.gantt!.syncTheme();
            done({
              css: getComputedStyle(document.querySelector(".gantt")!).getPropertyValue("--gantt-bg").trim().toLowerCase(),
              hook: window.gantt!.theme().bg,
            });
          }),
        );
      }),
  );
}

const expand = (hex: string): string => (/^#[0-9a-f]{3}$/.test(hex) ? `#${[...hex.slice(1)].map((c) => c + c).join("")}` : hex);

async function setTheme(page: Page, value: string | null): Promise<void> {
  await page.evaluate((v) => {
    const root = document.querySelector(".gantt")!;
    if (v === null) root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", v);
  }, value);
}

test("data_theme_forces_dark_or_light_whatever_the_system_says", async ({ g, page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await g.open({ n: 200 });
  expect(expand((await read(page)).css), "light system, no attribute").toBe(LIGHT_BG);
  await setTheme(page, "dark");
  let now = await read(page);
  expect(expand(now.css), "light system, data-theme=dark: the token").toBe(DARK_BG);
  expect(now.hook, "light system, data-theme=dark: the canvas theme").toBe(DARK_BG);

  await page.emulateMedia({ colorScheme: "dark" });
  await setTheme(page, "light");
  now = await read(page);
  expect(expand(now.css), "dark system, data-theme=light: the token").toBe(LIGHT_BG);
  expect(now.hook, "dark system, data-theme=light: the canvas theme").toBe(LIGHT_BG);

  await setTheme(page, null);
  now = await read(page);
  expect(expand(now.css), "dark system, no attribute: the system decides again").toBe(DARK_BG);
  expect(now.hook).toBe(DARK_BG);
});
