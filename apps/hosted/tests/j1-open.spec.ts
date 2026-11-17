import { expect, test } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product";
import { ROWS } from "../samples/office-fitout";
import { pageErrors, trackLogs, waitReady, waitSampleSettled } from "./support/page";
import { OWN_XER_NAMES, OWN_XER_ROWS, ownXer, truncatedXer } from "./support/xer";

// J1, open a schedule (gs#541, moved from the chart's own tests (j1-open.spec.ts), which served a file as `schedule.dat` to the developer
// page). On the hosted app there are two ways in, and both are checked here: a built-in sample chosen from the "Open a
// sample" menu, and your own file chosen with the chart's file chooser, which reads it in the browser and makes no request.

test("a sample chosen from the menu opens: the office fit-out's rows and names are on the chart", { tag: "@smoke" }, async ({ page }) => {
  const logs = trackLogs(page);
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });

  await page.locator("#open-button").click();
  await expect(page.locator("#sample-list")).toBeVisible();
  await page.locator('#sample-list button[data-sample-id="office-fitout"]').click();
  await waitSampleSettled(page, 120_000);

  await page.waitForFunction((rows) => window.gantt!.status().n === rows, ROWS.length, { timeout: 60_000 });
  const first = ROWS[0]!.name;
  const labels = await page.evaluate((count) => Array.from({ length: count }, (_, i) => window.gantt!.app.task_label(i)), ROWS.length);
  expect(labels, "the sample's names are on the chart").toContain(first);
  await expect(page.locator("#table-rows")).toContainText(first);
  expect(pageErrors(logs), "the page logs no error").toEqual([]);
});

test("your own file opens with the chooser, and no request is made for it", { tag: "@smoke" }, async ({ page }) => {
  const logs = trackLogs(page);
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });

  const requests: string[] = [];
  page.on("request", (request) => requests.push(`${request.method()} ${request.url()}`));
  await page.setInputFiles("#open-input", { name: "own.xer", mimeType: "application/octet-stream", buffer: ownXer() });
  await page.waitForFunction((rows) => window.gantt!.status().n === rows, OWN_XER_ROWS, { timeout: 60_000 });

  const labels = await page.evaluate((count) => Array.from({ length: count }, (_, i) => window.gantt!.app.task_label(i)), OWN_XER_ROWS);
  for (const name of OWN_XER_NAMES) expect(labels, `${name} is on the chart`).toContain(name);
  await expect(page.locator("#table-rows")).toContainText("XER A200");
  expect(requests, "opening a file makes no network request").toEqual([]);
  expect(pageErrors(logs), "the page logs no error").toEqual([]);
});

test("a truncated file shows the reader's error and leaves the chart as it was", async ({ page }) => {
  const logs = trackLogs(page);
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  const before = await page.evaluate(() => window.gantt!.status().n);

  await page.setInputFiles("#open-input", { name: "broken.xer", mimeType: "application/octet-stream", buffer: truncatedXer() });
  const error = page.locator("#open-error-text");
  await expect(error).toContainText("could not open broken.xer");
  expect(await error.textContent(), "the XER reader answers, not the XML one").not.toMatch(/XML error/i);
  await expect(error, "the line says which files open").toContainText(`${PRODUCT_NAME} opens P6 XER, MS Project XML and .gantt files.`);
  expect(await page.evaluate(() => window.gantt!.status().n), "the chart keeps its schedule").toBe(before);
  // The importer's refusal is shown to the person; it is not a page error. Only an uncaught exception fails here.
  expect(logs.filter((line) => line.startsWith("pageerror")), "no uncaught error").toEqual([]);
});
