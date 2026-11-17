// What the hosted specs that move from the chart's own tests (gs#541, S8) share: wait for the chart, collect the page's log, and
// wait for a sample to finish opening. These are the hosted app's own; nothing here imports packages/gantt/**.
import { readFileSync } from "node:fs";
import { expect, type Page } from "@playwright/test";
import { SAMPLES_DIR } from "../samples-file";

/** A page reload (the WebGPU to WebGL2 fallback) can destroy the execution context under a call; try again after the load. */
export async function acrossReload<T>(page: Page, run: () => Promise<T>, attempts = 3): Promise<T> {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await run();
    } catch (error) {
      if (attempt >= attempts || !/context was destroyed|navigat/i.test(String(error))) throw error;
      await page.waitForLoadState("load");
    }
  }
}

/** The chart is mounted and has drawn `frames` frames, and the page's fonts have settled. */
export async function waitReady(page: Page, { frames, timeout }: { frames: number; timeout: number }): Promise<void> {
  await acrossReload(page, () =>
    page.waitForFunction((k) => window.gantt?.ready && window.gantt.status().frames >= k, frames, { timeout }),
  );
  await acrossReload(page, () => page.evaluate(() => document.fonts.ready.then(() => undefined)));
}

/**
 * A file sample opens after the chart mounts, and the header says "Fetching <label>, <size>…" until it has. A built-in sample has
 * nothing to fetch. This waits only for that text to clear; it does not look at the schedule, so a scene that checks the
 * schedule still fails by its own name.
 */
export async function waitSampleSettled(page: Page, timeout: number): Promise<void> {
  await acrossReload(page, () =>
    page.waitForFunction(() => (document.getElementById("sample-status")?.textContent ?? "") === "", undefined, { timeout }),
  );
}

export function trackLogs(page: Page): string[] {
  const logs: string[] = [];
  page.on("console", (message) => logs.push(`${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => logs.push(`pageerror: ${error.message}`));
  return logs;
}

export function pageErrors(logs: string[]): string[] {
  return logs.filter((line) => /^(error|pageerror)/.test(line));
}

/**
 * Serve the sample files the way a static host that stores a `.gz` file as it is does: the gzip bytes, with no
 * `Content-Encoding`. `vite preview` serves a `.gz` file with `Content-Encoding: gzip`, so the browser undoes the
 * gzip before the page sees the bytes and the page's own `DecompressionStream` step never runs. The bytes are checked
 * to be gzip, so a test using this really reads packed data.
 */
export async function serveGzipAsStored(page: Page): Promise<void> {
  await page.route("**/samples/*.gz", async (route) => {
    const file = new URL(new URL(route.request().url()).pathname.split("/").pop()!, SAMPLES_DIR);
    const body = readFileSync(file);
    expect([body[0], body[1]], "the file on the wire is gzip").toEqual([0x1f, 0x8b]);
    await route.fulfill({ status: 200, body, headers: { "content-type": "application/gzip" } });
  });
}
