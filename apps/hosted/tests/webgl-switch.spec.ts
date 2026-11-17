import { expect, test } from "@playwright/test";

// The hosted page and a WebGPU failure (gs#1217). The chart switches to WebGL2 in its box by itself: the page stays, keeps the edits,
// puts its "Open…" menu back into the chart's new toolbar and names the new backend. The app's chrome (`apps/chrome`: the Filters
// menu and the find box) puts itself back into the new toolbar too, and still drives the chart. A panic leaves the chart stopped (a `fatal` error):
// the page then starts again by itself, so it is never left stuck.

interface Hook {
  ready?: boolean;
  app: { rename_task(task: number, name: string): void };
  failNextFrame(panic: boolean): void;
  status(): { frames: number; backend: string };
}

test("a WebGPU frame failure switches to WebGL2 without a reload, and the menu works in the new toolbar", { tag: "@smoke" }, async ({ page }) => {
  // Document loads, not navigations: the page puts `?sample=` in the address bar with `history.replaceState`, which is no reload.
  let loads = 0;
  page.on("load", () => {
    loads += 1;
  });
  // Arm the failure before the first frame (an init script polls for the hook), so the WebGPU frame that fails is the one armed here.
  await page.addInitScript(() => {
    const timer = setInterval(() => {
      const hook = (window as unknown as { gantt?: Hook }).gantt;
      if (hook?.ready !== true) return;
      clearInterval(timer);
      if (hook.status().backend === "WebGPU") {
        (window as unknown as { armed?: boolean }).armed = true;
        hook.failNextFrame(false);
      }
    }, 0);
  });
  await page.goto("/");
  test.skip(!(await page.evaluate(() => "gpu" in navigator)), "this browser has no WebGPU to lose");
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
  await page.waitForTimeout(500);
  expect(loads, "the page was loaded once and never reloaded").toBe(1);
  expect(await page.evaluate(() => (window as unknown as { armed?: boolean }).armed ?? false), "the chart started on WebGPU").toBe(true);
  expect(new URL(page.url()).searchParams.has("backend"), "no ?backend=webgl reload").toBe(false);
  await expect(page.locator("#backend-label")).toHaveText("WebGL2");

  // The menu is back in the new toolbar and opens from the new "Open…" button.
  await expect(page.locator("#toolbar-file #sample-list")).toHaveCount(1);
  await page.locator("#open-button").click();
  await expect(page.locator("#sample-list")).toBeVisible();
  await expect(page.locator("#open-button")).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("Escape");

  // The chrome is in the new toolbar, once, and drives the new app: Critical only filters the rows, and Find counts its matches.
  await expect(page.locator("#find #find-input")).toHaveCount(1);
  await expect(page.locator("#filters-menu #filters-button")).toHaveCount(1);
  await page.locator("#filters-button").click();
  await page.locator("#critical-toggle").click();
  await expect(page.locator("#critical-toggle")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#filters-button")).toHaveText("Filters · 1");
  await page.keyboard.press("Escape");
  await page.locator("#find-input").fill("a");
  await expect(page.locator("#find-count")).toHaveText(/ of |no matches/);
});

test("a panic in a frame reloads the page once; once the new start draws, a later panic reloads again", { tag: "@smoke" }, async ({ page }) => {
  const ready = (): Promise<unknown> =>
    page.waitForFunction(() => {
      const hook = (window as unknown as { gantt?: Hook }).gantt;
      return hook?.ready === true && hook.status().frames >= 5;
    }, undefined, { timeout: 120_000 });
  const panicReloads = async (): Promise<boolean> => {
    const reloaded = page.waitForEvent("load", { timeout: 30_000 }).then(() => true, () => false);
    await page.evaluate(() => (window as unknown as { gantt: Hook }).gantt.failNextFrame(true));
    return reloaded;
  };
  await page.goto("/?backend=webgl");
  await ready();
  expect(await panicReloads(), "the page started again after the panic").toBe(true);
  await ready();
  expect(new URL(page.url()).searchParams.get("restarted"), "the page knows it has started again").toBe("1");
  // The start after the restart succeeded and kept drawing: the page drops `?restarted=1`, so a later panic reloads once again.
  await page.waitForFunction(() => !new URLSearchParams(location.search).has("restarted"), undefined, { timeout: 30_000 });
  expect(await panicReloads(), "a later panic reloads again").toBe(true);
  await ready();
});

test("after a restart, a chart that panics soon after its start shows why instead of reloading again", { tag: "@smoke" }, async ({ page }) => {
  // Arm a panic as soon as the page that has already started again has its chart, well before its start counts as a success.
  await page.addInitScript(() => {
    const timer = setInterval(() => {
      const hook = (window as unknown as { gantt?: Hook }).gantt;
      if (hook?.ready !== true) return;
      clearInterval(timer);
      hook.failNextFrame(true);
    }, 0);
  });
  let loads = 0;
  page.on("load", () => {
    loads += 1;
  });
  await page.goto("/?backend=webgl&restarted=1");
  await expect(page.locator("[data-reason]")).toContainText("deliberate panic");
  await page.waitForTimeout(2000);
  expect(loads, "a page that already started again does not reload again").toBe(1);
  expect(new URL(page.url()).searchParams.get("restarted"), "the restart mark stays").toBe("1");
});
