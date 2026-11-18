// Checks the book's link, picture and media checks (scripts/lib/book-pages.ts). Run from anywhere: node scripts/test-book-pages.ts
// Needs Node 22.18 or later (type stripping). Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { checkAssets, MEDIA_BUDGET_BYTES, userPages } from "./lib/book-pages.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-book-pages: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};
// A book of two pages and one picture the first page shows; each case changes one thing.
const book = (a: string, extra: Record<string, number> = {}) => {
  const texts = new Map([["a.md", a], ["b.md", "# Part two\n"]]);
  const files = new Map([["a.md", 1], ["b.md", 1], ["media/p.webp", 1], ...Object.entries(extra)]);
  return checkAssets(texts, files);
};
const page = '[b](b.md#part-two)\n<img src="media/p.webp" alt="">\n';

check("a good book passes", book(page), []);
check("a broken page link fails", book(page + "[x](gone.md)\n"), ['a.md: "gone.md" points at a file that does not exist']);
check("a missing anchor fails", book(page + "[x](b.md#nope)\n"), ['a.md: "b.md#nope" points at a heading that does not exist']);
check("a missing image fails", book(page + "![x](media/gone.webp)\n"), ['a.md: "media/gone.webp" points at a file that does not exist']);
check("a missing video source fails", book(page + '<video poster="media/gone.webp"><source src="media/gone.mp4"></video>\n').length, 2);
check("a link in a fenced block and a URL are not checked", book(page + "```\n[x](gone.md)\n```\n[u](https://x.test/a.md)\n"), []);
check("a link to api/ fails", book(page + "[api](api/index.md)\n"), ['a.md: "api/index.md" points at a file that does not exist']);
check("an orphan file fails", book(page, { "media/orphan.webp": 1 }), ["media/orphan.webp: no page uses this file"]);
check("an include counts as use", book(page + "{{#include media/inc.svg}}\n", { "media/inc.svg": 1 }), []);
check("an over-budget file fails", book(page + '<img src="media/big.mp4">\n', { "media/big.mp4": MEDIA_BUDGET_BYTES + 1 }).length, 1);
check("a file at the budget passes", book(page + '<img src="media/big.mp4">\n', { "media/big.mp4": MEDIA_BUDGET_BYTES }), []);

// The developers part may come first: the user pages are the ones outside it, on either side.
const parts = "[Welcome](index.md)\n# For developers\n- [Dev](dev.md)\n# Guide\n- [Open](open.md)\n";
check("pages after the developers part are user pages", userPages(parts), { pages: ["index.md", "open.md"] });

const run = spawnSync(process.execPath, [join(import.meta.dirname, "check-book-pages.ts"), join(import.meta.dirname, "..")], { encoding: "utf8" });
check("today's book passes", [run.status, run.stderr], [0, ""]);

if (failed) process.exit(1);
console.log("test-book-pages: ok");
