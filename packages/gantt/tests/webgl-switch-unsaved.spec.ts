import { expect, test } from "@playwright/test";

// A switch to WebGL2 that cannot keep the schedule still starts with the unsaved mark on (gantt#2108), so the page's copy of the work is kept.
interface Hook {
  app: { rename_task(task: number, name: string): void; save: (name: string) => string };
  failNextFrame(panic: boolean): void;
  status(): { frames: number; backend: string };
}

test("a switch that cannot keep the schedule starts with the unsaved mark on", async ({ page }) => {
  await page.goto("/");
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
  await page.waitForFunction(() => (window as unknown as { gantt?: { ready?: boolean } }).gantt?.ready === true, undefined, { timeout: 120_000 });
  await page.evaluate(() => {
    const hook = (window as unknown as { gantt: Hook }).gantt;
    hook.app.rename_task(1, "Edited before the switch");
    hook.app.save = () => {
      throw new Error("save refused");
    };
    hook.failNextFrame(false);
  });
  await page.waitForFunction(
    () => {
      try {
        const hook = (window as unknown as { gantt?: Hook }).gantt;
        return hook?.status().backend === "WebGL2" && hook.status().frames >= 5;
      } catch {
        return false;
      }
    },
    undefined,
    { timeout: 120_000 },
  );
  await expect(page.locator("#unsaved-mark"), "the new start is unsaved").toBeVisible();
});
