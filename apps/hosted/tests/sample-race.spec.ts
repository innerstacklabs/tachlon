import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { SAMPLES_DIR, readSamplesJson } from "./samples-file";

// A newer sample pick cancels the older download, and a download that stalls ends with a sentence (gs#703).

interface Entry {
  id: string;
  label: string;
  file: string | null;
  rows: number;
}

const SAMPLES: Entry[] = readSamplesJson<{ samples: Entry[] }>().samples;
const COUNT = new Intl.NumberFormat("en-US");

function entry(id: string): Entry {
  const found = SAMPLES.find((sample) => sample.id === id);
  if (found === undefined) throw new Error(`samples.json has no ${id}`);
  return found;
}

async function ready(page: Page): Promise<void> {
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, {
    timeout: 180_000,
  });
}

async function pick(page: Page, id: string): Promise<void> {
  await page.click("#open-button");
  await page.click(`#sample-list button[data-sample-id="${id}"]`);
}

/** Hold the response for `sample`'s file until `release()`. */
async function hold(page: Page, sample: Entry): Promise<() => void> {
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route(`**/samples/${sample.file}`, async (route) => {
    await held;
    await route.fulfill({
      status: 200,
      body: readFileSync(new URL(sample.file!, SAMPLES_DIR)),
      headers: { "content-type": "application/gzip" },
    });
  });
  return release;
}

test("a first sample whose download finishes after the second has opened does not replace it", async ({ page }) => {
  const first = entry("large-50k");
  const second = entry("office-fitout");
  await page.goto("/");
  await ready(page);
  const release = await hold(page, first);
  await pick(page, first.id);
  await expect(page.locator("#sample-status")).toContainText("Fetching");
  await pick(page, second.id);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(second.rows)} rows`, { timeout: 60_000 });
  release();
  // Give the late first download time to arrive and (wrongly) open.
  await page.waitForTimeout(3_000);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(second.rows)} rows`);
  expect(new URL(page.url()).searchParams.get("sample")).toBe(second.id);
  await expect(page.locator("#sample-status")).toHaveText("");
});

test("a download that stalls past the limit says so and leaves the open chart alone", async ({ page }) => {
  const stalled = entry("large-50k");
  await page.clock.install();
  await page.goto("/");
  await ready(page);
  const before = await page.locator("#row-count").textContent();
  await page.route(`**/samples/${stalled.file}`, () => undefined); // never answered
  await pick(page, stalled.id);
  await expect(page.locator("#sample-status")).toContainText("Fetching");
  await page.clock.fastForward(31_000);
  await expect(page.locator("#sample-status")).toHaveText(
    `The sample "${stalled.label}" did not arrive within 30 seconds, so it was not opened. Open a file… still works.`,
  );
  await expect(page.locator("#row-count")).toHaveText(before ?? "");
});

test("a download started from the page's own start (?sample=) is cancelled by a pick during it", async ({ page }) => {
  const first = entry("large-50k");
  const second = entry("office-fitout");
  const release = await hold(page, first);
  await page.goto(`/?sample=${first.id}`);
  await expect(page.locator("#sample-status")).toContainText("Fetching");
  await pick(page, second.id);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(second.rows)} rows`, { timeout: 60_000 });
  release();
  await page.waitForTimeout(3_000);
  await expect(page.locator("#row-count")).toHaveText(`${COUNT.format(second.rows)} rows`);
  expect(new URL(page.url()).searchParams.get("sample")).toBe(second.id);
});
