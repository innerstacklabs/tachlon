// The `select` job's rule for a fast-tier run: whether the code jobs (rust, wasm, smoke, web, hosted, hosted-browser) run. `code` is false only
// when every changed file is a book chapter (`book/src/*.md`), one of the three book-page scripts, or a page directly under `docs/` that no
// source names; the `book` job then runs the chapter tests itself. `browser` is true on such a run when the book's contents page changed:
// `hosted-browser` reads the Welcome page's Next link from the built book, and needs the `wasm` bindings to build. Anything else, and anything
// it cannot read, says code true. Needs Node 22.18 or later (type stripping).
export interface DocsPlan {
  code: boolean;
  browser: boolean;
  why: string;
}

const BOOK_SCRIPTS = ["scripts/lib/book-pages.ts", "scripts/check-book-pages.ts", "scripts/test-book-pages.ts"];
const SUMMARY = "book/src/SUMMARY.md";
// Built from parts: the architecture tests scan scripts/ for these names, and allow neither in this file. The product's name may be written
// under book/ and in no other place a changed file can be, so a chapter is scanned for the old names only.
const OLD = [["gantt", "spike"].join("[-_]"), ["man", "tis"].join("")];
const oldNames = new RegExp(OLD.join("|"), "i");
const anyName = new RegExp([...OLD, ["tach", "lon"].join("")].join("|"), "i");

/** `named(path)`: whether a file outside scripts/, docs/, book/ and .github/ names `path`. `read(path)`: its text, null when the file is gone. */
export function docsOnly(changed: string[], named: (path: string) => boolean, read: (path: string) => string | null): DocsPlan {
  const code = (why: string): DocsPlan => ({ code: true, browser: false, why });
  if (changed.length === 0) return code("no changed files against main to judge");
  for (const f of changed) {
    const chapter = /^book\/src\/[^/]+\.md$/.test(f);
    const page = /^docs\/[^/]+\.md$/.test(f);
    if (!chapter && !page && !BOOK_SCRIPTS.includes(f)) return code(`${f} is no book chapter, book-page script or page under docs/`);
    if (f === "docs/WASM_API.md") return code(`${f} is read by the architecture tests`);
    if (page && named(f)) return code(`${f} is named by a test or source`);
    const scan = chapter ? oldNames : anyName;
    if (scan.test(f) || scan.test(read(f) ?? "")) return code(`${f} names an old project name or the product, which the architecture tests scan for`);
  }
  return { code: false, browser: changed.includes(SUMMARY), why: `only book chapters, book-page scripts and docs pages no test names changed: ${changed.join(" ")}` };
}
