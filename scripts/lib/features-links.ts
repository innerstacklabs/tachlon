// The features page's drift check (apps/site/features.html): each of its doc links, `<a class="doc" href>`, must point at a doc
// this repo holds: a heading of the package README (the public docs on npm) or a chapter of the book (the guide).
// Each table row's `data-matrix` number names its row of docs/features.md, and the row's product cell must open with that row's verdict.
// Called by scripts/features-links.ts; tested by scripts/test-features-links.ts.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** The slugs of the headings in a markdown text, outside code fences: lower case, punctuation dropped, spaces to hyphens. */
export function headingSlugs(md: string): Set<string> {
  const text = md.replace(/^```[\s\S]*?^```/gm, "");
  const heads = [...text.matchAll(/^#{1,6}\s+(.+?)\s*$/gm)].map((m) => m[1]);
  return new Set(heads.map((h) => h.toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s+/g, "-")));
}

/** The `href` of every `<a class="doc">` in a page. */
export const docLinks = (html: string): string[] => [...html.matchAll(/<a\s[^>]*class="doc"[^>]*href="([^"]*)"/g)].map((m) => m[1]);

/** The doc links of `html` that name no section: a heading the README lacks, a chapter or heading the book lacks, or another host. `root` is the repo root. */
export function brokenLinks(html: string, root: string): string[] {
  const name: string = JSON.parse(readFileSync(join(root, "packages", "gantt", "package.json"), "utf8")).name; // the product's name is written there, not here
  const npm = `https://www.npmjs.com/package/${name}#`;
  const book = `https://app.${name}.com/book/`;
  const hasHeading = (file: string, slug: string): boolean => existsSync(file) && (!slug || headingSlugs(readFileSync(file, "utf8")).has(slug));
  return docLinks(html).filter((href) => {
    if (href.startsWith(npm)) return !hasHeading(join(root, "packages", "gantt", "README.md"), href.slice(npm.length) || "-");
    const m = href.startsWith(book) ? /^([\w-]+)\.html(?:#([\w-]+))?$/.exec(href.slice(book.length)) : null;
    return !m || !hasHeading(join(root, "book", "src", `${m[1]}.md`), m[2] ?? "");
  });
}

/** The product's verdict (`Yes`, `In progress` or `No`) of each numbered row of the table in docs/features.md. */
export const matrixVerdicts = (md: string): Map<number, string> =>
  new Map([...md.matchAll(/^\|\s*(\d+)\s*\|[^|]*\|\s*(Yes|In progress|No)\b/gm)].map((m): [number, string] => [Number(m[1]), m[2]]));

/** The page rows (`<tr data-matrix="n">`) whose product cell opens with another verdict than matrix row n of `md` (docs/features.md). */
export function matrixMismatches(html: string, md: string): string[] {
  const want = matrixVerdicts(md);
  return [...html.matchAll(/<tr data-matrix="(\d+)">.*?<\/th><td>(.*?)<\/td>/g)].flatMap(([, n, cell]) => {
    const got = /^(Yes|In progress|No)\b/.exec(cell.replace(/<[^>]*>/g, "").trim())?.[1];
    return got !== undefined && got === want.get(Number(n)) ? [] : [`matrix row ${n}: page says ${got ?? "no verdict"}, matrix says ${want.get(Number(n)) ?? "no such row"}`];
  });
}
