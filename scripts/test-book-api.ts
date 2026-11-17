// Checks the book's API preprocessor (scripts/book-api.ts). Run from anywhere: node scripts/test-book-api.ts
// Needs Node 22.18 or later (type stripping). Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-book-api: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};
const script = join(import.meta.dirname, "book-api.ts");
const run = (env: Record<string, string>, ...args: string[]) =>
  spawnSync(process.execPath, [script, ...args], { encoding: "utf8", input: JSON.stringify([{}, { items: [] }]), env: { ...process.env, ...env } });
type Item = { Chapter: { name: string; path: string; sub_items: Item[] } };
const shape = (items: Item[]): unknown[] => items.map(({ Chapter: c }) => [c.name, c.path, shape(c.sub_items)]);

const dir = mkdtempSync(join(tmpdir(), "book-api-"));
// With the extension kept, "pkg.bryntumresult.md" sorts before "pkg.md": the parent must still come first.
for (const [file, title] of [["pkg.mount.md", "pkg.mount() function"], ["index.md", "API Reference"], ["pkg.md", "pkg package"], ["pkg.bryntumresult.md", "BryntumResult"]]) {
  writeFileSync(join(dir, file), `<!-- generated -->\n\n## ${title}\n`);
}
const out = run({ BOOK_API_PAGES: dir });
const mount = ["pkg.mount() function", "api/pkg.mount.md", []];
const pkg = ["pkg package", "api/pkg.md", [["BryntumResult", "api/pkg.bryntumresult.md", []], mount]];
check("pages nest by their dotted file name", [out.status, shape(JSON.parse(out.stdout).items)], [0, [["API Reference", "api/index.md", [pkg]]]]);
check("BOOK_API=skip leaves the book as it is", JSON.parse(run({ BOOK_API: "skip", BOOK_API_PAGES: join(dir, "none") }).stdout).items, []);
const missing = run({ BOOK_API_PAGES: join(dir, "none") });
check("missing pages exit 1 and name the fix", [missing.status, missing.stderr.includes("pnpm docs:book")], [1, true]);
check("supports every renderer", run({}, "supports", "html").status, 0);

if (failed) process.exit(1);
console.log("test-book-api: ok");
