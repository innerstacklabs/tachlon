import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// gantt#1790: `s` with focus in the chart shows the open time and the frame numbers on the hosted page, `s` again hides them, and the
// line floats over the chart's edge, so the chart's box is the same either way. Pictures of the shown line in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

test("s shows the open time and frame numbers; s again hides them", async ({ page }, info) => {
  // WebGL2 from the start: a WebGPU start that fails switches the chart to WebGL2 in place, and `Gantt.speed().open` is null after that switch.
  await page.goto("/?backend=webgl");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const line = page.locator("#speed-line");
  const box = (): Promise<unknown> => page.locator("#timeline").boundingBox();
  await expect(line, "hidden until asked for").toBeHidden();
  const before = await box();

  await page.locator("#app").focus();
  await page.keyboard.press("s");
  await expect(line).toBeVisible();
  await expect(line, "the open's time").toHaveText(/open .+ \d[\d,.]* ms/);
  expect(await box(), "the chart's canvas box is the same with the line shown").toEqual(before);

  // Scroll for about two seconds so the frame window fills, counting the line's text writes.
  const rect = await page.locator("#timeline").boundingBox();
  await page.mouse.move(rect!.x + rect!.width / 2, rect!.y + rect!.height / 2);
  await page.evaluate(() => {
    const w = window as unknown as { writes: number; since: number };
    w.writes = 0;
    w.since = performance.now();
    new MutationObserver((records) => (w.writes += records.length)).observe(document.getElementById("speed-line")!, { childList: true, characterData: true, subtree: true });
  });
  for (let i = 0; i < 40; i += 1) {
    await page.mouse.wheel(0, i % 2 === 0 ? 120 : -120);
    await page.waitForTimeout(50);
  }
  await expect(line, "the frame numbers").toContainText("p50");
  // The limit follows the time the page measured, not the loop's two seconds: a busy runner stretches the loop and legally allows more writes.
  const { writes, elapsed } = await page.evaluate(() => {
    const w = window as unknown as { writes: number; since: number };
    return { writes: w.writes, elapsed: performance.now() - w.since };
  });
  expect(writes, `text writes in ${Math.round(elapsed)} ms, at most five a second and one more`).toBeLessThanOrEqual(Math.floor(elapsed / 200) + 1);

  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    // The chart takes the new colours on the next frame and draws them on the one after.
    const seen = await page.evaluate(() => window.gantt!.status().frames);
    await page.waitForFunction((from) => window.gantt!.status().frames >= from + 2, seen);
    await page.screenshot({ path: info.outputPath(`speed-line-${scheme}-1280.png`) });
  }
  await page.locator("#app").focus();
  await page.keyboard.press("s");
  await expect(line, "s again hides it").toBeHidden();
  expect(await box(), "the chart's canvas box is the same with the line hidden again").toEqual(before);
});
