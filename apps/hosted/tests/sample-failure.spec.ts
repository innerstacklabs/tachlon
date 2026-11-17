import { expect, test } from "@playwright/test";
import { serveGzipAsStored } from "./support/page";

// A sample or the catalog that cannot be fetched says why, in words, and that "Open a file…" still works (gs#703).
// The status line is `#sample-status`. A built-in schedule has nothing to fetch, so these use a file sample:
// `?sample=medium-5k` fetches `samples/medium-5k.xer.gz` as soon as the chart is up.

const STATUS = "#sample-status";
const STILL_WORKS = /Open a file… still works\.$/;

test("no connection to a sample's file: the line says there is no connection", async ({ page }) => {
  await page.route("**/samples/*.gz", (route) => route.abort("connectionfailed"));
  await page.goto("/?sample=medium-5k");
  await expect(page.locator(STATUS)).toHaveText(/^medium-5k did not open: there is no connection, so the sample "Medium: 5,000 activities \(generated XER\)" could not be fetched\./);
  await expect(page.locator(STATUS)).toHaveText(STILL_WORKS);
});

test("a sample's file the server does not have: the line says it is missing and gives the status", async ({ page }) => {
  await page.route("**/samples/*.gz", (route) => route.fulfill({ status: 404 }));
  await page.goto("/?sample=medium-5k");
  await expect(page.locator(STATUS)).toHaveText(/^medium-5k did not open: the sample .* is missing on the server \(HTTP 404\)\./);
  await expect(page.locator(STATUS)).toHaveText(STILL_WORKS);
});

test("a server error on a sample's file: the line says the server had an error", async ({ page }) => {
  await page.route("**/samples/*.gz", (route) => route.fulfill({ status: 503 }));
  await page.goto("/?sample=medium-5k");
  await expect(page.locator(STATUS)).toHaveText(/^medium-5k did not open: the server had an error fetching the sample .* \(HTTP 503\)\./);
  await expect(page.locator(STATUS)).toHaveText(STILL_WORKS);
});

test("a catalog that cannot be read says the same, and the Open… button still opens the file chooser", async ({ page }) => {
  await page.route("**/samples/samples.json", (route) => route.abort("connectionfailed"));
  // The page is asked for WebGL2. When WebGPU fails (on a runner with no GPU it always does) the page reloads once after its
  // first frame (`reloadOnWebgl` in src/main.ts), and a test that ends while that reload is under way closes its context
  // during a navigation; on a loaded runner the close then waited out the whole 4 minute test timeout (gs#1126).
  await page.goto("/?backend=webgl");
  await expect(page.locator(STATUS)).toHaveText(/^There is no connection, so the sample list could not be fetched\./);
  await expect(page.locator(STATUS)).toHaveText(STILL_WORKS);
  await expect(page.locator("#open-button")).not.toHaveAttribute("aria-haspopup", "menu");
});

test("a catalog the server does not have gives the status too", async ({ page }) => {
  await page.route("**/samples/samples.json", (route) => route.fulfill({ status: 404 }));
  await page.goto("/");
  await expect(page.locator(STATUS)).toHaveText(/^The sample list is missing on the server \(HTTP 404\)\. Open a file… still works\.$/);
});

test("any other status on a sample's file: the line says the server refused it", async ({ page }) => {
  await page.route("**/samples/*.gz", (route) => route.fulfill({ status: 403 }));
  await page.goto("/?sample=medium-5k");
  await expect(page.locator(STATUS)).toHaveText(/^medium-5k did not open: the server refused the sample .* \(HTTP 403\)\. Open a file… still works\.$/);
});

test("a catalog that is not JSON says it could not be read", async ({ page }) => {
  await page.route("**/samples/samples.json", (route) => route.fulfill({ status: 200, contentType: "application/json", body: "<html>" }));
  await page.goto("/");
  await expect(page.locator(STATUS)).toHaveText(/^The sample list could not be read \(it is not valid JSON\)\. Open a file… still works\.$/);
});

test("an empty catalog says the list is empty", async ({ page }) => {
  await page.route("**/samples/samples.json", (route) => route.fulfill({ status: 200, contentType: "application/json", body: '{"samples":[]}' }));
  await page.goto("/");
  await expect(page.locator(STATUS)).toHaveText(/^The sample list is empty\. Open a file… still works\.$/);
});

test("a browser with no DecompressionStream says it cannot unpack the sample and points to Open a file…", async ({ page }) => {
  await page.addInitScript(() => {
    delete (window as { DecompressionStream?: unknown }).DecompressionStream;
  });
  await serveGzipAsStored(page);
  await page.goto("/");
  await page.waitForFunction(() => window.gantt?.ready === true, undefined, { timeout: 180_000 });
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="medium-5k"]`);
  await expect(page.locator(STATUS)).toHaveText("medium-5k did not open: This browser cannot unpack the sample; use Open a file…");
});
