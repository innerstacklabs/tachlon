// Checks the feature matrix's drift check (see scripts/lib/features-matrix.ts). Run from anywhere: node scripts/test-features-matrix.ts
// Needs Node 22.18 or later (type stripping). The repo is a temporary directory copied from this one. Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-features-matrix: FAIL "${name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`);
};

const repo = join(import.meta.dirname, "..");
const cli = join(import.meta.dirname, "features-matrix.ts");
const run = (root: string, mode: string) => spawnSync(process.execPath, [cli, mode, "--root", root], { encoding: "utf8" }).status;
const root = mkdtempSync(join(tmpdir(), "features-matrix-"));
mkdirSync(join(root, "docs"));
mkdirSync(join(root, "packages", "gantt", "src"), { recursive: true });
cpSync(join(repo, "docs", "features.md"), join(root, "docs", "features.md"));
cpSync(join(repo, "README.md"), join(root, "README.md"));
cpSync(join(repo, "packages", "gantt", "src", "product.ts"), join(root, "packages", "gantt", "src", "product.ts"));
const readme = readFileSync(join(root, "README.md"), "utf8");
const edit = (file: string, from: string, to: string): void => writeFileSync(join(root, file), readFileSync(join(root, file), "utf8").replace(from, to));

check("the real README's table is docs/features.md's table", spawnSync(process.execPath, [cli, "--check"]).status, 0);
edit("README.md", "| Four link types (FS, SS, FF, SF) with lag or lead | Yes |", "| Four link types (FS, SS, FF, SF) with lag or lead | No |");
check("a README whose table differs by one cell fails", run(root, "--check"), 1);
check("--write puts the table back", [run(root, "--write"), run(root, "--check")], [0, 0]);
check("--write leaves the rest of the README alone", readFileSync(join(root, "README.md"), "utf8"), readme);
edit("docs/features.md", "| Critical path | Yes |", "| Critical path | No |");
check("a changed cell in docs/features.md fails", run(root, "--check"), 1);
writeFileSync(join(root, "README.md"), readme.replace("<!-- features-matrix:end -->", ""));
check("a README with no end marker fails", run(root, "--check"), 1);

const features = readFileSync(join(repo, "docs", "features.md"), "utf8");
const counts = /^\*\*Counts\.\*\*.*$/m.exec(features)?.[0] ?? "(no Counts line)";
writeFileSync(join(root, "README.md"), readme);
writeFileSync(join(root, "docs", "features.md"), features.replace(counts, "**Counts.** 48 rows. This chart: 1 Yes."));
check("a wrong Counts line fails", run(root, "--check"), 1);
check("--write repairs the Counts line", [run(root, "--write"), run(root, "--check")], [0, 0]);
const both = () => ["docs/features.md", "README.md"].map((f) => readFileSync(join(root, f), "utf8"));
check("--write leaves every other line of both files alone", both(), [features, readme]);
edit("docs/features.md", "| Critical path | Yes |", "| Critical path | No |");
edit("README.md", "| Critical path | Yes |", "| Critical path | No |");
check("a flipped cell with an untouched Counts line fails", run(root, "--check"), 1);
process.exit(failed ? 1 : 0);
