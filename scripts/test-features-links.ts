// Checks the features page's drift check (see scripts/lib/features-links.ts). Run from anywhere: node scripts/test-features-links.ts
// Needs Node 22.18 or later (type stripping). The repo is a temporary directory of two small docs. Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { brokenLinks, matrixMismatches } from "./lib/features-links.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-features-links: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};

const repo = join(import.meta.dirname, "..");
const name: string = JSON.parse(readFileSync(join(repo, "packages", "gantt", "package.json"), "utf8")).name;
const root = mkdtempSync(join(tmpdir(), "features-links-"));
mkdirSync(join(root, "packages", "gantt"), { recursive: true });
mkdirSync(join(root, "book", "src"), { recursive: true });
writeFileSync(join(root, "packages", "gantt", "package.json"), JSON.stringify({ name }));
writeFileSync(join(root, "packages", "gantt", "README.md"), "# Pkg\n\n## Load a P6 or MS Project file\n\n```sh\n# not a heading\n```\n");
writeFileSync(join(root, "book", "src", "j5-print.md"), "# Print\n\n## By hand\n");
const npm = `https://www.npmjs.com/package/${name}#`;
const book = `https://app.${name}.com/book/`;
const a = (href: string): string => `<a class="doc" href="${href}">doc</a>`;
const good = [a(`${npm}load-a-p6-or-ms-project-file`), a(`${book}j5-print.html`), a(`${book}j5-print.html#by-hand`), `<a href="${npm}gone">not a doc link</a>`].join("\n");
check("links to a README heading and a book chapter pass", brokenLinks(good, root), []);
const dead = [`${npm}gone`, `${npm}not-a-heading`, `${npm}`, `${book}gone.html`, `${book}j5-print.html#gone`, "https://example.com/x"];
check("a dead heading, a dead chapter, a dead book heading, a bare README link and another host are each broken", brokenLinks(dead.map(a).join("\n"), root), dead);

const cli = join(import.meta.dirname, "features-links.ts");
const page = (html: string): string => (writeFileSync(join(root, "p.html"), html), join(root, "p.html"));
const run = (file: string) => spawnSync(process.execPath, [cli, file, "--root", root], { encoding: "utf8" }).status;
check("the CLI exits 0 for a page whose links all exist", run(page(good)), 0);
check("the CLI exits 1 for a page with a dead link", run(page(good + a(`${npm}gone`))), 1);
check("the CLI exits 1 for a page with no doc link, and 2 for a page it cannot read", [run(page("<p>none</p>")), run(join(root, "none.html"))], [1, 2]);
mkdirSync(join(root, "docs"), { recursive: true });
writeFileSync(join(root, "docs", "features.md"), "| # | Feature | This chart |\n|---|---|---|\n| | **Scheduling** | |\n| 1 | Links | Yes | All |\n| 2 | Splits | No | Ent only |\n| 3 | Colors | In progress (#1) | All |\n");
const row = (n: number, cell: string): string => `<tr data-matrix="${n}"><th scope="row">F</th><td><a href="x">${cell}</a></td><td>Yes</td></tr>`;
const rows = (...r: string[]): string => good + r.join("\n");
check("a row whose product cell opens with the matrix's verdict passes", matrixMismatches(rows(row(1, "Yes, XML"), row(2, "No"), row(3, "In progress")), readFileSync(join(root, "docs", "features.md"), "utf8")), []);
check("a row that says Yes where the matrix says No, a row with no verdict and a row with no matrix row are each a mismatch", matrixMismatches(rows(row(2, "Yes"), row(1, "Not yet"), row(9, "Yes")), readFileSync(join(root, "docs", "features.md"), "utf8")).length, 3);
check("the CLI exits 0 for a page whose rows agree with docs/features.md", run(page(rows(row(1, "Yes"), row(2, "No")))), 0);
check("the CLI exits 1 for a page that says Yes where docs/features.md says No", run(page(rows(row(1, "Yes"), row(2, "Yes")))), 1);
check("the real features page names a matrix row on its table rows", readFileSync(join(repo, "apps", "site", "features.html"), "utf8").includes("<tr data-matrix="), true);
check("the real features page has no broken link", spawnSync(process.execPath, [cli, join(repo, "apps", "site", "features.html")]).status, 0);
process.exit(failed ? 1 : 0);
