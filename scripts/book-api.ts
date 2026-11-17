// An mdBook preprocessor (book/book.toml): adds the generated API reference to the book as chapters. mdBook renders only the chapters its
// book names, and the reference has one page per export and member, so a SUMMARY.md entry for each would go stale with every export. The pages
// are the ones `pnpm docs:book` writes in packages/gantt (api-documenter markdown, `pkg.mount.md`); each becomes a chapter titled by its
// first `## ` heading, nested under the page whose file name is its own minus the last dotted part (`pkg.mount` under `pkg`, which sits
// under `index`). Needs Node 22.18 or later (type stripping). Exits 1, with the reason, when the pages are missing.
//   BOOK_API_PAGES  the pages folder (default packages/gantt/dist-docs/pages)
//   BOOK_API=skip   leave the book as it is (a docs-only CI run builds without the wasm that the pages need)
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

if (process.argv[2] === "supports") process.exit(0);
const [, book] = JSON.parse(readFileSync(0, "utf8"));
if (process.env.BOOK_API !== "skip") {
  const dir = process.env.BOOK_API_PAGES ?? join(import.meta.dirname, "../packages/gantt/dist-docs/pages");
  const files = (() => {
    try {
      return readdirSync(dir).filter((f) => f.endsWith(".md")).map((f) => f.slice(0, -3)).sort();
    } catch {
      return [];
    }
  })();
  if (files.length === 0) {
    console.error(`book-api: no pages in ${dir}; run \`pnpm docs:book\` in packages/gantt first (it needs the generated src/wasm/)`);
    process.exit(1);
  }
  const chapters = new Map<string, { sub_items: unknown[] }>();
  for (const key of files) {
    const content = readFileSync(join(dir, `${key}.md`), "utf8");
    const path = `api/${key}.md`;
    const chapter = { name: /^## (.+)$/m.exec(content)?.[1] ?? key, content, number: null, sub_items: [], path, source_path: path, parent_names: [] };
    const parent = key === "index" ? undefined : chapters.get(key.includes(".") ? key.slice(0, key.lastIndexOf(".")) : "index");
    (parent?.sub_items ?? book.items).push({ Chapter: chapter });
    chapters.set(key, chapter);
  }
}
console.log(JSON.stringify(book));
