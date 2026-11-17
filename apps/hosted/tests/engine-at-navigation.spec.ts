import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// gs#1336: the page asks for its engine (the .wasm file) as it opens, not after it has read the sample list, and asks for it once.

test("the engine's request starts while the sample list is still held, and is made once", async ({ page }) => {
  const wasm: string[] = [];
  let wasmFirst: boolean | undefined;
  let seen!: () => void;
  const asked = new Promise<boolean>((resolve) => {
    seen = () => resolve(true);
    setTimeout(() => resolve(false), 5_000);
  });
  page.on("request", (request) => {
    if (!request.url().endsWith(".wasm")) return;
    wasm.push(request.url());
    seen();
  });
  await page.route("**/samples/samples.json", async (route) => {
    wasmFirst = await asked;
    await route.continue();
  });
  await page.goto("/");
  await waitReady(page, { frames: 1, timeout: 180_000 });
  expect(wasmFirst, "the engine was asked for before the sample list was let through").toBe(true);
  await expect(page.locator("link[rel=preload][as=fetch]"), "the built page preloads the engine").toHaveAttribute("href", /\.wasm$/);
  expect(wasm, "one request for the engine").toHaveLength(1);
  const timed = await page.evaluate(() => performance.getEntriesByType("resource").filter((entry) => entry.name.endsWith(".wasm")).length);
  expect(timed, "one wasm entry in the resource timing").toBe(1);
});
