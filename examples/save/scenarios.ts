import assert from "node:assert/strict";
import type { Page } from "@playwright/test";
import type { FakeServer, Script } from "./server.ts";

/** A scenario of decision 0022, rule 11.2. `keys` pins the key mode it runs in; without it the suite runs both. */
export interface Scenario { n: number; name: string; keys?: "chart" | "server"; script: Script; run(page: Page, server: FakeServer): Promise<void> }

const until = async (what: string, ok: () => boolean, ms: number): Promise<void> => {
  for (const end = Date.now() + ms; !ok(); await new Promise((resolve) => setTimeout(resolve, 50))) if (Date.now() > end) assert.fail(`timed out waiting for ${what}`);
};

export const SCENARIOS: Scenario[] = [
  {
    n: 1, name: "1 slow saves", script: { latency: 800 },
    async run(page, server) {
      await page.locator('#table-rows .row[data-row="2"]').waitFor({ timeout: 30_000 });
      for (const [row, days] of [[0, "7"], [1, "8"], [2, "9"]]) {
        await page.locator(`#table-rows .row[data-row="${row}"] .dur`).dblclick();
        await page.locator("#table-rows input.cell-edit").fill(days!);
        await page.keyboard.press("Enter");
      }
      const writes = () => server.seen.filter((r) => r.method !== "GET");
      await until("three saves", () => writes().length >= 3, 8_000);
      assert.deepEqual(writes().map((r) => [r.key, r.body?.days]), [["t1", 7], ["t2", 8], ["t3", 9]], "three calls, in commit order");
      assert.equal(server.maxInFlight, 1, "never two at once");
    },
  },
];
