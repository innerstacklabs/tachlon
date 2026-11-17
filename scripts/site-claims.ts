// Whether the pages may publish: `node scripts/site-claims.ts <dir|file.html>...` (Node 22.18+). Prints
// `file:line: stories` per claim left; exits 0 when none is left, 1 when some are, 2 when a path or a page
// cannot be read (or none is given). The rule is in scripts/lib/site-claims.ts.
import { siteClaims } from "./lib/site-claims.ts";

const paths = process.argv.slice(2);
if (paths.length === 0) {
  console.error("site-claims: usage: node scripts/site-claims.ts <dir|file.html>...");
  process.exit(2);
}
let found: ReturnType<typeof siteClaims>;
try {
  found = siteClaims(paths);
} catch (e) {
  console.error(`site-claims: cannot read: ${(e as Error).message}`);
  process.exit(2);
}
for (const c of found.claims) console.log(`${c.file}:${c.line}: ${c.stories.join(" ") || "(no story named)"}`);
console.error(`site-claims: ${found.claims.length} claim(s) left in ${found.files.length} page(s)`);
process.exit(found.claims.length === 0 ? 0 : 1);
