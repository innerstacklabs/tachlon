// Checks which fast-tier runs skip the code jobs (see scripts/lib/docs-only.ts). Run from the repo root:
//   node scripts/test-docs-only.ts
// Needs Node 22.18 or later (type stripping). A table of changed files against `code` and `browser`, with the files a source names and the
// text each file holds as stand-ins for the search and the read. Exits 1, one line per problem.
import { docsOnly } from "./lib/docs-only.ts";

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-docs-only: FAIL ${line}`);
};

// The names are built from parts, as in the rule: the architecture tests scan this file for them.
const OLD = ["gantt", "spike"].join("-");
const BRAND = ["tach", "lon"].join("");
const KEYS = "docs/KEYS.md";
const PR42 = ["book/src/SUMMARY.md", "book/src/dev-data.md", "book/src/dev-look.md", "book/src/dev-page.md", "book/src/dev-save.md", "scripts/lib/book-pages.ts", "scripts/test-book-pages.ts"];

const rows: Array<{ name: string; changed: string[]; text?: Record<string, string>; code: boolean; browser?: boolean }> = [
  { name: "the seven files of PR 42 skip the code jobs and keep hosted-browser", changed: PR42, code: false, browser: true },
  { name: "chapters without the contents page skip them and keep nothing", changed: ["book/src/dev-data.md", "scripts/check-book-pages.ts"], code: false, browser: false },
  { name: "the contents page alone keeps hosted-browser", changed: ["book/src/SUMMARY.md"], code: false, browser: true },
  { name: "a contents page that names an old project name runs the code jobs", changed: ["book/src/SUMMARY.md"], text: { "book/src/SUMMARY.md": `- [${OLD}](x.md)` }, code: true, browser: false },
  { name: "a chapter that names an old project name runs them", changed: ["book/src/dev-data.md"], text: { "book/src/dev-data.md": `built as ${OLD}` }, code: true },
  { name: "a chapter whose file name holds an old project name runs them", changed: [`book/src/${OLD}.md`], code: true },
  { name: "a chapter that names the product skips them", changed: ["book/src/index.md"], text: { "book/src/index.md": `${BRAND} draws it` }, code: false },
  { name: "a page under docs/ that names the product runs them", changed: ["docs/DEMOS.md"], text: { "docs/DEMOS.md": `${BRAND} draws it` }, code: true },
  { name: "a book-page script that names the product runs them", changed: ["scripts/lib/book-pages.ts"], text: { "scripts/lib/book-pages.ts": `// ${BRAND}` }, code: true },
  { name: "a page a source names runs them", changed: [KEYS], code: true },
  { name: "a page no source names skips them", changed: ["docs/DEMOS.md"], code: false },
  { name: "a chapter and a page a source names run them", changed: ["book/src/index.md", KEYS], code: true },
  { name: "the wasm API page runs them, named or not", changed: ["docs/WASM_API.md"], code: true },
  { name: "a decision record runs them", changed: ["docs/decisions/0013-x.md"], code: true },
  { name: "the book's stylesheet runs them", changed: ["book/theme/book.css"], code: true },
  { name: "the book's config runs them", changed: ["book/book.toml"], code: true },
  { name: "a chapter folder's media file runs them", changed: ["book/src/media/j2.webp"], code: true },
  { name: "a markdown file in the media folder runs them", changed: ["book/src/media/notes.md"], code: true },
  { name: "a .ts file outside the three scripts runs them", changed: ["book/src/dev-data.md", "scripts/lib/touched-specs.ts"], code: true },
  { name: "the workflow runs them", changed: [".github/workflows/ci.yml"], code: true },
  { name: "an empty diff runs them", changed: [], code: true },
  { name: "a deleted chapter skips them", changed: ["book/src/j3-gone.md"], code: false },
];
for (const row of rows) {
  const got = docsOnly(row.changed, (path) => path === KEYS, (path) => row.text?.[path] ?? null);
  if (got.code !== row.code || (row.browser !== undefined && got.browser !== row.browser))
    fail(`"${row.name}": got code=${got.code} browser=${got.browser} (${got.why}), wanted code=${row.code}${row.browser === undefined ? "" : ` browser=${row.browser}`}`);
}

// A file the rule cannot read stops it: the error reaches scripts/docs-only.ts, which says code true.
let threw = false;
try {
  docsOnly(["book/src/dev-data.md"], () => false, () => { throw new Error("unreadable"); });
} catch {
  threw = true;
}
if (!threw) fail("a chapter that cannot be read is judged anyway");

if (failed) process.exit(1);
console.log("test-docs-only: ok");
