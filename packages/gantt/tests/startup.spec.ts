import { expect, test, waitReady } from "./support/gantt";
import * as knobs from "./support/knobs";

test("page generates the demo schedule", { tag: "@smoke" }, async ({ g }) => {
  await g.open();
  const s0 = await g.status();
  const fallback = await g.page.evaluate(() => window.gantt!.fallback);
  console.log(`backend ${s0.backend}, ${s0.n} tasks, surface ${s0.surface.join("x")}, dpr ${knobs.dpr}`);
  if (fallback) console.log(`WebGPU fell back to WebGL2: ${fallback}`);
  if (knobs.backend === "webgl") {
    expect.soft(s0.backend === "WebGL2", "forced WebGL2 backend is in use").toBe(true);
  }
  expect.soft(s0.n === knobs.n, `page generated ${knobs.n} tasks`).toBe(true);
});

test("init panic on auto: page recovers on WebGL2", async ({ page }) => {
  const base = `${knobs.url}?n=2000&panic=init`;
  await page.goto(base);
  try {
    await waitReady(page, { frames: 3, timeout: 30_000 });
    const result = await page.evaluate(() => ({
      backend: window.gantt!.status().backend,
      fallback: window.gantt!.fallback,
      search: location.search,
    }));
    expect
      .soft(
        result.backend === "WebGL2" &&
          /deliberate panic/.test(result.fallback ?? "") &&
          !new URLSearchParams(result.search).has("panic"),
        `init panic on auto: page recovered on WebGL2 with the reason (${result.fallback})`,
      )
      .toBe(true);
  } catch (error) {
    expect
      .soft(false, `init panic on auto: page did not recover within 30 s (${String(error).split("\n")[0]})`)
      .toBe(true);
  }
});

test("init panic on forced WebGL2 shows the error", async ({ page }) => {
  const base = `${knobs.url}?n=2000&panic=init`;
  await page.goto(`${base}&backend=webgl`);
  try {
    await page.waitForFunction(
      () => document.getElementById("overlay")?.textContent?.startsWith("render error:"),
      null,
      { timeout: 30_000 },
    );
    const result = await page.evaluate(() => ({
      text: document.getElementById("overlay")!.textContent,
      search: location.search,
      gantt: Boolean(window.gantt),
    }));
    expect
      .soft(
        /deliberate panic/.test(result.text ?? "") && result.search.includes("backend=webgl") && !result.gantt,
        `init panic on forced WebGL2: error shown, no reload (${result.text})`,
      )
      .toBe(true);
  } catch (error) {
    expect
      .soft(
        false,
        `init panic on forced WebGL2: no render error shown within 30 s (${String(error).split("\n")[0]})`,
      )
      .toBe(true);
  }
});
