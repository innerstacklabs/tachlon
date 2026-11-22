// The generated regions of apps/site/index.html (between the <!-- bench:NAME --> marks) equal what scripts/bench-table.ts writes. A hand edit, or a change to packages/gantt/releases.json, demos/bench/versions.json or the results file with no regeneration, fails here.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { DEFAULT_RESULTS } from "../../../scripts/bench-table.ts";
import { methodBlock, withBenchTable } from "../../../scripts/lib/bench-table.ts";

const read = (path: string | URL): string => readFileSync(typeof path === "string" ? new URL(`../../../${path}`, import.meta.url) : path, "utf8");

test("the generated regions of the site page equal the generator's output", () => {
  const page = read("apps/site/index.html");
  const made = withBenchTable(page, JSON.parse(read(DEFAULT_RESULTS)), JSON.parse(read("demos/bench/versions.json")));
  expect(made === page, "the page differs from the generator: run `node scripts/bench-table.ts` and commit the page; never edit text between <!-- bench:NAME --> marks by hand").toBe(true);
});

test("the method note states the matched pairs of each cell from the results file, never a fixed count", () => {
  const results = JSON.parse(read(DEFAULT_RESULTS));
  const counts: Record<string, number> = { "tree4-10000": 4, "chain-10000": 6, "tree4-50000": 5, "chain-50000": 7 };
  for (const [cell, matched] of Object.entries(counts)) results.cells[cell].matched = matched;
  const note = methodBlock(results, JSON.parse(read("demos/bench/versions.json")));
  expect(note).toContain("Matched pairs per cell: 10,000 tasks 4 / 6, 50,000 tasks 5 / 7; at least 4 in every cell.");
  expect(note).not.toMatch(/median of \d+ runs/);
});
