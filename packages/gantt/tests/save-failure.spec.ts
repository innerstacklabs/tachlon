// A failed save shows its cause in the alert line (gs#703): the gzip step rejects, and the page says so.
import { expect, test } from "./support/gantt";

test("save_failure_shows_its_cause_in_the_alert_line", async ({ g }) => {
  const { page } = g;
  await page.addInitScript(() => {
    Object.defineProperty(window, "CompressionStream", { value: undefined, configurable: true });
  });
  await g.open({ n: 20 });
  await page.locator("#app").focus();
  await page.keyboard.press("ControlOrMeta+S");
  const line = page.locator("#edit-error");
  await expect(line).toBeVisible();
  await expect(page.locator("#edit-error-text")).toContainText("Not saved");
});
