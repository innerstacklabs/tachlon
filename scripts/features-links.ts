// Whether the features page's doc links all name a section that exists: `node scripts/features-links.ts <page.html> [--root <repo>]`
// (Node 22.18+). Prints each broken link, and each `data-matrix` row whose product cell disagrees with docs/features.md; exits 0 when none is
// broken, 1 when some are or the page has no doc link, 2 when the page, or the matrix a page with `data-matrix` rows needs, cannot be read.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brokenLinks, docLinks, matrixMismatches } from "./lib/features-links.ts";

const args = process.argv.slice(2);
const i = args.indexOf("--root");
const root = i >= 0 ? args.splice(i, 2)[1] : join(import.meta.dirname, "..");
let html: string;
try {
  html = readFileSync(args[0] ?? "", "utf8");
} catch (e) {
  console.error(`features-links: cannot read: ${(e as Error).message}`);
  process.exit(2);
}
let broken = brokenLinks(html, root);
if (html.includes("data-matrix=")) {
  try {
    broken = [...broken, ...matrixMismatches(html, readFileSync(join(root, "docs", "features.md"), "utf8"))];
  } catch (e) {
    console.error(`features-links: cannot read: ${(e as Error).message}`);
    process.exit(2);
  }
}
for (const href of broken) console.log(href);
const total = docLinks(html).length;
console.error(`features-links: ${broken.length} problem(s), ${total} doc link(s)`);
process.exit(broken.length === 0 && total > 0 ? 0 : 1);
