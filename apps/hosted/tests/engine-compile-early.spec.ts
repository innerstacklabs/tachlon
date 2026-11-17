import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// gs#1354: the page compiles the engine from its inline script and hands the module to the chart; without it the chart loads the file itself.

test("the engine is compiled by the page's inline script and the chart starts on that module", async ({ page }) => {
  await page.addInitScript(() => {
    const original = WebAssembly.instantiateStreaming;
    (window as unknown as { streamed: number }).streamed = 0;
    WebAssembly.instantiateStreaming = (...args) => {
      (window as unknown as { streamed: number }).streamed++;
      return original.apply(WebAssembly, args);
    };
  });
  await page.goto("/");
  await waitReady(page, { frames: 1, timeout: 180_000 });
  expect(await page.evaluate(() => (window as unknown as { __engineModule?: unknown }).__engineModule instanceof Promise), "the inline script kept the compile").toBe(true);
  expect(await page.evaluate(() => (window as unknown as { streamed: number }).streamed), "init compiled nothing itself").toBe(0);
});

test("a failed early compile falls back to the chart's own load", async ({ page }) => {
  await page.addInitScript(() => {
    WebAssembly.compileStreaming = () => Promise.reject(new Error("no early compile"));
    const original = WebAssembly.instantiateStreaming;
    (window as unknown as { streamed: number }).streamed = 0;
    WebAssembly.instantiateStreaming = (...args) => {
      (window as unknown as { streamed: number }).streamed++;
      return original.apply(WebAssembly, args);
    };
  });
  await page.goto("/");
  await waitReady(page, { frames: 1, timeout: 180_000 });
  expect(await page.evaluate(() => document.getElementById("start-failure") === null), "the chart started").toBe(true);
  expect(await page.evaluate(() => (window as unknown as { streamed: number }).streamed), "init fetched and compiled the file itself").toBe(1);
});
