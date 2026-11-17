import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Print..." item (gs#703), under "Export the list as CSV": choosing it closes the menu and calls window.print once.
test.use({ viewport: { width: 1280, height: 800 } });

test("the Print item closes the menu and calls window.print once", async ({ page }) => {
  await page.addInitScript(() => {
    window.print = () => void ((window as unknown as { prints: number }).prints += 1);
  });
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  for (const how of ["click", "keyboard"] as const) {
    await page.evaluate(() => ((window as unknown as { prints: number }).prints = 0));
    await page.locator("#open-button").click();
    const item = page.getByRole("menuitem", { name: /Print\.\.\./ });
    await expect(item).toBeVisible();
    await expect(item, "it names its paper, so it and Print on A4 read as a pair").toContainText("On US Letter paper");
    await expect(item, "live, not Coming soon").not.toHaveAttribute("aria-disabled", "true");
    if (how === "click") await item.click();
    else await item.focus().then(() => page.keyboard.press("Enter"));
    await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
    expect(await page.evaluate(() => (window as unknown as { prints: number }).prints), `${how}: window.print called once`).toBe(1);
  }
});
