// Fill the speed table and the two inline figures on the page from a results file: `node scripts/bench-table.ts [results.json]` (Node 22.18+).
// Default: the file demos/bench/results/current names, the one file that holds every cell. Rewrites apps/site/index.html in place.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { withBenchTable } from "./lib/bench-table.ts";

const read = (p: string) => JSON.parse(readFileSync(new URL(p, import.meta.url), "utf8"));
const dir = new URL("../demos/bench/results/", import.meta.url);
/** The results file the page shows: the file name on the one line of demos/bench/results/current. */
export const DEFAULT_RESULTS = new URL(readFileSync(new URL("current", dir), "utf8").trim(), dir);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const results = process.argv[2] ? JSON.parse(readFileSync(process.argv[2], "utf8")) : JSON.parse(readFileSync(DEFAULT_RESULTS, "utf8"));
  const page = new URL("../apps/site/index.html", import.meta.url);
  writeFileSync(page, withBenchTable(readFileSync(page, "utf8"), results, read("../demos/bench/versions.json")));
}
