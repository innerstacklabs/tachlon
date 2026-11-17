import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// A page that cannot start says so in a readable block (gs#703). Two ways the engine cannot start: the wasm file does not load,
// and the browser has neither WebGL2 nor WebGPU. Each shows a centered `role="alert"` block that names the reason and what the page needs,
// not the chart's small corner line.

test.use({ viewport: { width: 1280, height: 800 } });

test("when the wasm file does not load the page shows an alert block that says it could not start and what it needs", async ({ page }) => {
  await page.route("**/*.wasm", (route) => route.abort());
  await page.goto("/");
  const alert = page.locator('[role="alert"]:not([hidden])');
  await expect(alert).toBeVisible();
  await expect(alert).toContainText("could not start");
  await expect(alert, "it says what the page needs").toContainText(/WebGL2 or WebGPU/);
  await expect(alert, "it names the reason").toContainText(/fetch|load|wasm|network/i);
  const box = await alert.boundingBox();
  expect(box, "the block has a box").not.toBeNull();
  const middle = box!.x + box!.width / 2;
  expect(Math.abs(middle - 1280 / 2), "the block is centered across the page").toBeLessThan(40);
});

test("when the browser has neither WebGL2 nor WebGPU the page shows the same block within the engine's watchdog", async ({ page }) => {
  await page.addInitScript(() => {
    HTMLCanvasElement.prototype.getContext = () => null;
    delete (Navigator.prototype as { gpu?: unknown }).gpu;
    delete (navigator as { gpu?: unknown }).gpu;
  });
  await page.goto("/");
  const alert = page.locator('[role="alert"]:not([hidden])');
  // The first try is WebGPU, which fails and reloads on WebGL2 (?backend=webgl); that one fails too and ends in the block.
  await expect(alert).toBeVisible({ timeout: 40_000 });
  await expect(alert).toContainText("could not start");
  await expect(alert).toContainText(/WebGL2 or WebGPU/);
  expect(new URL(page.url()).searchParams.get("backend"), "WebGL2 was tried before giving up").toBe("webgl");
});

test("a page that starts has no alert block and nothing over the chart (the hidden template draws nothing)", async ({ page }) => {
  await page.goto("/");
  // Fails fast on a template that CSS has made visible (a rule on the block's id also matches a `<template>` that carries the same id):
  // every template is in the DOM from the first byte, whatever its id.
  const templates = await page.evaluate(() =>
    [...document.querySelectorAll("template")].map((el) => ({ id: el.id, display: getComputedStyle(el).display })),
  );
  expect(templates.length, "the page has templates to check").toBeGreaterThan(0);
  expect(
    templates.filter((t) => t.display !== "none"),
    "every template stays display:none",
  ).toEqual([]);
  await waitReady(page, { frames: 1, timeout: 60_000 });
  await expect(page.locator("#start-failure")).toHaveCount(0);
  const top = await page.evaluate(() => document.elementFromPoint(640, 400)?.tagName ?? "");
  expect(top, "the point at the middle of the page is the chart, not a template or an overlay").not.toBe("TEMPLATE");
});
