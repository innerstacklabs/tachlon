// Checks the book's user pages (see scripts/lib/book-pages.ts). Run from the repo root:
//   node scripts/check-book-pages.ts [repo-root]
// Needs Node 22.18 or later (type stripping). Exits 1 and prints one line per problem.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { checkAssets, checkBook } from "./lib/book-pages.ts";

const root = process.argv[2] ?? ".";
const src = join(root, "book/src");
const files = new Map<string, number>();
const texts = new Map<string, string>();
for (const rel of readdirSync(src, { recursive: true, encoding: "utf8" })) {
  const path = rel.split("\\").join("/");
  const stat = statSync(join(src, rel));
  if (!stat.isFile()) continue;
  files.set(path, stat.size);
  if (path.endsWith(".md")) texts.set(path, readFileSync(join(src, rel), "utf8"));
}
const problems = [
  ...checkBook(
    readFileSync(join(src, "SUMMARY.md"), "utf8"),
    readFileSync(join(root, "apps/hosted/scripts/build-samples.mjs"), "utf8"),
    (page) => (existsSync(join(src, page)) ? readFileSync(join(src, page), "utf8") : null),
  ),
  ...checkAssets(texts, files),
];
for (const p of problems) console.error(`check-book-pages: ${p}`);
if (problems.length > 0) process.exit(1);
console.log("check-book-pages: ok");
