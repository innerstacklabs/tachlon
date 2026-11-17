import { expect, test } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product";
import { waitReady } from "./support/page";

// gs#703: the page says it is loading until the app mounts, and says it needs JavaScript when there is none.

test("while the sample list is still loading, #app says it is loading the product", async ({ page }) => {
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/samples/samples.json", async (route) => {
    await held;
    await route.continue();
  });
  await page.goto("/", { waitUntil: "commit" });
  await expect(page.locator("#app")).toContainText(`Loading ${PRODUCT_NAME}`);
  release();
});

test.describe("without JavaScript", () => {
  test.use({ javaScriptEnabled: false });

  test("the page says it needs JavaScript", async ({ page }) => {
    await page.goto("/");
    // Chromium still hides <noscript> under this emulation, so the check reads the document, not the screen.
    expect(await page.content()).toMatch(/<noscript>[^]*This page needs JavaScript[^]*<\/noscript>/);
  });
});

test("the loading line is there first, and once the app is ready it is gone", async ({ page }) => {
  // The test waits for the app to be ready, so it takes waitReady's budget, not the 30 s default
  // (ruling A devteam-1-352: on a busy runner the default ran out before the app mounted).
  test.setTimeout(180_000);
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/samples/samples.json", async (route) => {
    await held;
    await route.continue();
  });
  // The page is asked for WebGL2 so that it does not reload once more after its first frame, which is what it does when WebGPU
  // fails (`reloadOnWebgl` in src/main.ts, and on a runner with no GPU it always fails). A reload would put the loading line
  // back, and this test would be asserting on a page it did not start (gs#1124).
  await page.goto("/?backend=webgl", { waitUntil: "commit" });
  await expect(page.locator("#app")).toContainText(`Loading ${PRODUCT_NAME}`);
  release();
  await waitReady(page, { frames: 1, timeout: 180_000 });
  await expect(page.locator("#app")).not.toContainText(`Loading ${PRODUCT_NAME}`);
  // The loading line is the one paragraph that sits directly in #app. A mounted chart has other `p[role=status]` elements
  // deeper in (the Export dialog's result line, hidden until used), so a count over all of `#app p[role=status]` is 1 on a
  // ready page; the old selector read 0 only in the instants between the mount's first write and the Export dialog.
  await expect(page.locator("#app > p[role=status]")).toHaveCount(0);
});
