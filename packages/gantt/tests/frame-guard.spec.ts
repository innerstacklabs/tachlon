import { expect, test, waitReady, type Gantt } from "./support/gantt";

// A frame that throws must not kill the page (gs#163). The failures are forced through the test-only
// `failNextFrame` export. A forced WebGL2 run is used because an automatic WebGPU run answers a failed
// frame by reloading on WebGL2, which is a different path (startup.spec.ts).

const SIZE = 2000;
const FORCED = { backend: "webgl" };

/** The logs the deliberate failures write (the shell's console.error and the panic hook's); the fixture fails a test on any other. */
function forget(g: Gantt): void {
  const rest = g.logs.filter((line) => !/frame failed|deliberate/.test(line));
  g.logs.splice(0, g.logs.length, ...rest);
}

test("a frame that returns an error shows the banner, keeps the loop running and the page responds", async ({ g }) => {
  await g.open({ n: SIZE, params: FORCED });
  const banner = g.page.locator("#frame-error");
  await expect(banner).toBeHidden();

  await g.page.evaluate(() => window.gantt!.failNextFrame(false));
  await expect(banner).toBeVisible();
  await expect(g.page.locator("#frame-error-text")).toContainText("deliberate frame error");
  await expect(g.page.locator("#frame-error-reload"), "one failure is not the end: no reload yet").toBeHidden();

  // The loop kept running: frames moved on after the failure, and the chart still answers input.
  const before = (await g.status()).frames;
  await g.nextFrame(5);
  expect((await g.status()).frames, "the loop kept drawing").toBeGreaterThan(before);
  await expect(banner, "the banner says the chart is drawing again").toContainText("drawing again");
  const input = g.page.locator("#toolbar #find-input");
  await input.fill("a");
  await expect(g.page.locator("#find-count")).not.toHaveText("");

  await g.page.locator("#frame-error-dismiss").click();
  await expect(banner).toBeHidden();
  forget(g);
});

test("a panic in a frame shows the reload state at once, and the reload recovers", async ({ g }) => {
  await g.open({ n: SIZE, params: FORCED });
  await g.page.evaluate(() => window.gantt!.failNextFrame(true));

  const banner = g.page.locator("#frame-error");
  await expect(banner).toBeVisible();
  await expect(g.page.locator("#frame-error-text")).toContainText("deliberate panic in frame");
  await expect(g.page.locator("#frame-error-text")).toContainText("Reload to recover");
  await expect(g.page.locator("#frame-error-dismiss"), "the final state is not dismissable").toBeHidden();
  const reload = g.page.locator("#frame-error-reload");
  await expect(reload).toBeVisible();

  // The loop stopped asking for frames: the module is unusable, and the page itself still answers a click.
  await g.page.waitForTimeout(500);
  expect(await g.page.evaluate(() => document.getElementById("timeline") !== null)).toBe(true);
  forget(g);

  await Promise.all([g.page.waitForNavigation(), reload.click()]);
  await waitReady(g.page, { frames: 5, timeout: 60_000 });
  await expect(g.page.locator("#frame-error"), "a fresh page has no banner").toBeHidden();
});

test("three failed frames in a row stop the retries and show the reload state", async ({ g }) => {
  await g.open({ n: SIZE, params: FORCED });
  // A frame that keeps failing without a panic: the instance's own method hides the prototype's, and the loop calls it.
  await g.page.evaluate(() => {
    (window as unknown as { calls: number }).calls = 0;
    window.gantt!.app.frame = () => {
      (window as unknown as { calls: number }).calls += 1;
      throw new Error("stuck frame");
    };
  });

  await expect(g.page.locator("#frame-error-text")).toContainText("Reload to recover");
  await expect(g.page.locator("#frame-error-text")).toContainText("stuck frame");
  await expect(g.page.locator("#frame-error-reload")).toBeVisible();
  await g.page.waitForTimeout(500);
  const calls = await g.page.evaluate(() => (window as unknown as { calls: number }).calls);
  expect(calls, "frame() is tried three times and then left alone").toBe(3);
  forget(g);
});
