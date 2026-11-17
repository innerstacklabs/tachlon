// Checks the site's claim count (see scripts/lib/site-claims.ts). Run from anywhere:
//   node scripts/test-site-claims.ts
// Needs Node 22.18 or later (type stripping). Pages are inline strings written to a temporary directory.
// Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claimsIn } from "./lib/site-claims.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-site-claims: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};

const page = `<p>One</p>\n<a class="cta" data-claim="DNS" href="#">Try</a>\n<article\n  data-claim='tl#1089  tl#1094'>\n<span data-claim=tl#1205 data-claims="x" data-claim-tag="y">\n<i data-claim>`;
check("each attribute, its line and its stories", claimsIn("a.html", page).map((c) => [c.line, c.stories]), [[2, ["DNS"]], [4, ["tl#1089", "tl#1094"]], [5, ["tl#1205"]], [6, []]]);
check("a page with none, and the CSS selector, count nothing", claimsIn("b.html", `<p class="cloud">[data-claim]</p>`), []);

// apps/site/llms.txt: every link names a page the site ships (launch.html is removed before publishing) or one of two package URLs.
const site = join(import.meta.dirname, "..", "apps", "site");
const pkg: string = JSON.parse(readFileSync(join(site, "..", "..", "packages", "gantt", "package.json"), "utf8")).name; // the product's name is written there, not here
const home = `https://${pkg}.com/`;
const packageLinks = [`https://www.npmjs.com/package/${pkg}`, `https://unpkg.com/${pkg}/AGENTS.md`];
const linksIn = (text: string): string[] => [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const missingLinks = (text: string): string[] =>
  linksIn(text).filter((url) => {
    if (packageLinks.includes(url)) return false;
    const path = url.startsWith(home) ? url.slice(home.length) || "index.html" : "";
    return path === "" || path === "launch.html" || !existsSync(join(site, path.includes(".") ? path : `${path}.html`)); // the host serves a path with no extension from <path>.html
  });
const llms = existsSync(join(site, "llms.txt")) ? readFileSync(join(site, "llms.txt"), "utf8") : "";
check("llms.txt links name a shipped page or a package URL", missingLinks(llms), []);
check("llms.txt links the home page, how-it-works and both package URLs", linksIn(llms), [home, `${home}how-it-works`, ...packageLinks]);
check("a missing page, the launch page and another host are each missing", missingLinks(`[a](${home}gone.html) [b](${home}launch.html) [c](https://example.com/)`), [`${home}gone.html`, `${home}launch.html`, "https://example.com/"]);

const cli = join(import.meta.dirname, "site-claims.ts");
const run = (...args: string[]) => spawnSync(process.execPath, [cli, ...args], { encoding: "utf8" });
const dir = mkdtempSync(join(tmpdir(), "site-claims-"));
mkdirSync(join(dir, "clean", "deep"), { recursive: true });
writeFileSync(join(dir, "clean", "index.html"), "<p>True.</p>");
writeFileSync(join(dir, "clean", "deep", "notes.txt"), page);
writeFileSync(join(dir, "launch.html"), page);
mkdirSync(join(dir, "nested", "deep"), { recursive: true });
writeFileSync(join(dir, "nested", "deep", "x.html"), `<b data-claim="tl#1">`);
check("a clean directory exits 0", run(join(dir, "clean")).status, 0);
const left = run(join(dir, "clean"), join(dir, "launch.html"), join(dir, "nested"));
check("claims left exit 1", left.status, 1);
check("each claim left is listed", left.stdout.split("\n").filter(Boolean).length, 5);
check("a nested page is listed with file, line and stories", left.stdout.includes(`${join(dir, "nested", "deep", "x.html")}:1: tl#1`), true);
check("a missing path exits 2", run(join(dir, "gone.html")).status, 2);
check("no path exits 2", run().status, 2);

if (failed) process.exit(1);
console.log("test-site-claims: ok");
