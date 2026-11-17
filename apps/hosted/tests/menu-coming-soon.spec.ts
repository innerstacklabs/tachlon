import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu shows the save and export paths that are not built yet as greyed-out "Coming soon" items (gs#815):
// Save to cloud and Share a link. They are in the menu for assistive tech (aria-disabled, with the label), they do nothing when clicked,
// and the toolbar still fits at 1280 px. Screenshots of the open menu in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

const ITEMS = ["Save to cloud", "Share a link"];

test("the file menu lists Save to cloud and Share a link as disabled Coming soon items, and a click on one changes nothing", async ({ page }, info) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);

  await page.locator("#open-button").click();
  const list = page.locator("#sample-list");
  await expect(list).toBeVisible();

  const soon = list.locator('[role="menuitem"][aria-disabled="true"]');
  await expect(soon, "two disabled items").toHaveCount(ITEMS.length);
  for (const name of ITEMS) {
    const item = list.getByRole("menuitem", { name: new RegExp(name) });
    await expect(item, `${name} is in the menu`).toHaveCount(1);
    await expect(item).toHaveAttribute("aria-disabled", "true");
    await expect(item, `${name} says Coming soon`).toContainText("Coming soon");
  }

  const before = await page.evaluate(() => ({ n: window.gantt!.status().n, url: location.href, status: document.getElementById("sample-status")!.textContent }));
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  for (const name of ITEMS) await list.getByRole("menuitem", { name: new RegExp(name) }).click({ force: true }); // force: Playwright will not click an aria-disabled item, a person can
  await expect(list, "the menu stays open").toBeVisible();
  const after = await page.evaluate(() => ({ n: window.gantt!.status().n, url: location.href, status: document.getElementById("sample-status")!.textContent }));
  expect(after, "a click on a Coming soon item changes nothing").toEqual(before);
  expect(requests, "and makes no request").toEqual([]);

  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.screenshot({ path: info.outputPath(`file-menu-${scheme}-1280.png`), clip: { x: 0, y: 0, width: 1280, height: 420 } });
  }
});

test("in a short window the file menu ends inside the window and scrolls to its last item (gantt#1613)", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 500 });
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.locator("#open-button").click();
  const list = page.locator("#sample-list");
  const bottom = async (el: typeof list): Promise<number> => (await el.boundingBox().then((box) => box!.y + box!.height));
  expect(await bottom(list), "the list ends inside the window").toBeLessThanOrEqual(500);
  const last = list.getByRole("menuitem", { name: /Share a link/ });
  await last.scrollIntoViewIfNeeded();
  expect(await bottom(last), "the last item scrolls into view").toBeLessThanOrEqual(await bottom(list));
});
