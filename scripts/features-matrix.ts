// Whether the README's feature table is docs/features.md's table: `node scripts/features-matrix.ts --check [--root <repo>]` (Node 22.18+).
// It also checks the **Counts.** line under the table against the table (countsLine). `--write` copies the table into the README and rewrites
// that line. Exits 0 when all agree, 1 when they differ or a marker or the Counts line is missing, 2 when a file cannot be read.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { countsLine, productName, readmeBlock, renderBlock, withBlock } from "./lib/features-matrix.ts";

const args = process.argv.slice(2);
const i = args.indexOf("--root");
const root = i >= 0 ? args.splice(i, 2)[1] : join(import.meta.dirname, "..");
const mode = args[0];
if (mode !== "--check" && mode !== "--write") {
  console.error("usage: node scripts/features-matrix.ts --check|--write [--root <repo>]");
  process.exit(2);
}
let block: string, readme: string, features: string;
try {
  features = readFileSync(join(root, "docs", "features.md"), "utf8");
  block = renderBlock(features, productName(root));
  readme = readFileSync(join(root, "README.md"), "utf8");
} catch (e) {
  console.error(`features-matrix: cannot read: ${(e as Error).message}`);
  process.exit(2);
}
const next = withBlock(readme, block);
if (next === null) {
  console.error("features-matrix: README.md lacks the begin or end marker of the feature table");
  process.exit(1);
}
const counts = countsLine(features);
const oldCounts = features.split("\n").find((l) => l.startsWith("**Counts.**"));
if (oldCounts === undefined) {
  console.error("features-matrix: docs/features.md has no **Counts.** line under its table");
  process.exit(1);
}
if (mode === "--write") {
  writeFileSync(join(root, "README.md"), next);
  writeFileSync(join(root, "docs", "features.md"), features.replace(oldCounts, counts));
} else {
  const tableOk = readmeBlock(readme) === block;
  if (!tableOk) console.error("features-matrix: the README table differs from docs/features.md; run node scripts/features-matrix.ts --write");
  if (oldCounts !== counts) console.error(`features-matrix: docs/features.md's Counts line should read "${counts}"; run node scripts/features-matrix.ts --write`);
  if (!tableOk || oldCounts !== counts) process.exit(1);
}
