// Fails when a public export of the package has no doc comment, reading the doc model API Extractor wrote (tl#1094).
//   node scripts/api-docs-check.ts <doc model, packages/gantt/dist-docs/model/package.api.json>
// Exit 0: every export has one. 1: one FAIL line per export without. 2: the model cannot be read (missing, not JSON, no exports).
import { readFileSync } from "node:fs";
import { undocumentedExports } from "./lib/api-docs-check.ts";

const file = process.argv[2];
if (!file) {
  console.error("usage: node scripts/api-docs-check.ts <doc model>");
  process.exit(2);
}
try {
  const missing = undocumentedExports(readFileSync(file, "utf8"));
  for (const name of missing) console.error(`api-docs-check: FAIL ${name} is a public export with no doc comment`);
  if (missing.length) process.exit(1);
  console.log(`api-docs-check: PASS ${file}`);
} catch (e) {
  console.error(`api-docs-check: ${(e as Error).message}`);
  process.exit(2);
}
