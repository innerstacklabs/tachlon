// Checks the API docs check (see scripts/lib/api-docs-check.ts) through its CLI, on a doc model API Extractor wrote for a two-export
// `.d.ts` (scripts/fixtures/api-docs-model.json: `mount` documented, `unmount` not).
//   node scripts/test-api-docs-check.ts
// Exits 1, one FAIL line per problem.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-api-docs-check: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};
const run = (file: string) => spawnSync("node", [join(import.meta.dirname, "api-docs-check.ts"), file], { encoding: "utf8" });

const fixture = join(import.meta.dirname, "fixtures/api-docs-model.json");
const dir = mkdtempSync(join(tmpdir(), "test-api-docs-check-"));
try {
  // One export without a comment: exit 1, and the output names it and only it.
  const bad = run(fixture);
  check("undocumented export exits 1", bad.status, 1);
  check("names the undocumented export", bad.stderr.split("\n").filter((l) => l.includes("FAIL")).map((l) => l.split(" ")[2]), ["unmount"]);

  // The same model with a comment on it: exit 0. A comment with only a tag and no summary does not count.
  const model = JSON.parse(readFileSync(fixture, "utf8"));
  model.members[0].members[1].docComment = "/**\n * Take the chart out of its root element.\n */\n";
  writeFileSync(join(dir, "ok.json"), JSON.stringify(model));
  check("all documented exits 0", run(join(dir, "ok.json")).status, 0);
  model.members[0].members[1].docComment = "/**\n * @deprecated\n */\n";
  writeFileSync(join(dir, "tag-only.json"), JSON.stringify(model));
  check("tag-only comment exits 1", run(join(dir, "tag-only.json")).status, 1);

  // No model, not JSON, a model with no exports: exit 2 (it could not read, which is not a pass).
  check("missing input exits 2", run(join(dir, "none.json")).status, 2);
  writeFileSync(join(dir, "text.json"), "not json");
  check("not JSON exits 2", run(join(dir, "text.json")).status, 2);
  model.members[0].members = [];
  writeFileSync(join(dir, "empty.json"), JSON.stringify(model));
  check("no exports exits 2", run(join(dir, "empty.json")).status, 2);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("test-api-docs-check: PASS");
