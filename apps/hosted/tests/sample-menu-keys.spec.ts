import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The Open menu is operable with the keyboard (gs#703): ArrowDown and ArrowUp move focus over the menu items and wrap, Home and End go
// to the first and last, Escape closes the menu and returns focus to the Open button, and Tab out of the menu closes it.

test.use({ viewport: { width: 1280, height: 800 } });

const focused = (page: Page): Promise<string> =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    return el === null ? "" : el.id === "open-button" ? "#open-button" : (el.querySelector(".sample-label")?.textContent ?? el.id ?? "");
  });

async function openMenu(page: Page): Promise<void> {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.locator("#open-button").focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#sample-list")).toBeVisible();
}

test("ArrowDown, ArrowUp, Home and End move focus over the menu items, and wrap", async ({ page }) => {
  await openMenu(page);
  const items = await page.locator('#sample-list [role="menuitem"]:not([aria-disabled="true"]) .sample-label').allTextContents();
  expect(items[0]).toBe("New schedule");
  expect(items[1]).toBe("Open a file…");
  expect(items.length).toBeGreaterThan(2);

  await page.keyboard.press("ArrowDown");
  expect(await focused(page)).toBe(items[0]);
  await page.keyboard.press("ArrowDown");
  expect(await focused(page)).toBe(items[1]);
  await page.keyboard.press("ArrowUp");
  expect(await focused(page)).toBe(items[0]);
  await page.keyboard.press("ArrowUp");
  expect(await focused(page), "ArrowUp from the first wraps to the last").toBe(items[items.length - 1]);
  await page.keyboard.press("ArrowDown");
  expect(await focused(page), "ArrowDown from the last wraps to the first").toBe(items[0]);
  await page.keyboard.press("End");
  expect(await focused(page)).toBe(items[items.length - 1]);
  await page.keyboard.press("Home");
  expect(await focused(page)).toBe(items[0]);
});

test("ArrowDown walks every enabled item in order, the link and the export and print items included, and skips the disabled ones (gantt#1697)", async ({ page }) => {
  await openMenu(page);
  const items = await page.locator('#sample-list [role="menuitem"]:not([aria-disabled="true"]) .sample-label').allTextContents();
  for (const name of ["Open another schedule in a new tab", "Export the list as CSV", "Save image (PNG)", "Print...", "Print on A4...", "Save layout...", "Apply layout..."]) {
    expect(items, `${name} is an enabled item`).toContain(name);
  }
  for (const name of items) {
    await page.keyboard.press("ArrowDown");
    expect(await focused(page), "ArrowDown lands on the next item in menu order").toBe(name);
  }
  expect(items[items.length - 1], "the walk ends on the last layout item").toBe("Apply layout...");
  await page.keyboard.press("ArrowDown");
  expect(await focused(page), "past the last item it wraps to the first").toBe(items[0]);
});

test("Escape closes the menu and puts focus back on Open", async ({ page }) => {
  await openMenu(page);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  expect(await focused(page), "focus is on the second item before Escape").not.toBe("#open-button");
  await page.keyboard.press("Escape");
  await expect(page.locator("#sample-list")).toBeHidden();
  expect(await focused(page)).toBe("#open-button");
});

test("Tab out of the menu closes it", async ({ page }) => {
  await openMenu(page);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Tab");
  await expect(page.locator("#sample-list")).toBeHidden();
  await expect(page.locator("#open-button")).toHaveAttribute("aria-expanded", "false");
});
