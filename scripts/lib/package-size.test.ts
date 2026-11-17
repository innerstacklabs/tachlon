// The package size budget's exit codes (tl#1091): over budget 1, at budget 0, no budget file 2, a pack report with no size 2.
//   node scripts/lib/package-size.test.ts
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { tarballBytes } from "./package-size.ts";

const cli = join(import.meta.dirname, "..", "package-size.ts");
const root = mkdtempSync(join(tmpdir(), "package-size-test-"));
let failed = false;
try {
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: "fixture", version: "1.0.0", files: ["a.txt"] }));
  writeFileSync(join(root, "a.txt"), "x".repeat(5000));
  const size = tarballBytes(root);
  const run = (): number | null => spawnSync("node", [cli, root], { encoding: "utf8" }).status;
  const budget = (bytes: number): void => writeFileSync(join(root, "size-budget.json"), JSON.stringify({ bytes }));
  const want = (name: string, got: number | null, code: number): void => {
    if (got === code) return;
    failed = true;
    console.error(`package-size.test: FAIL ${name}: exit ${got}, want ${code}`);
  };
  want("no budget file", run(), 2);
  budget(size - 1);
  want("1 byte over budget", run(), 1);
  budget(size);
  want("at budget", run(), 0);
  // A pack report with no `size` must fail, not read as "not over budget".
  mkdirSync(join(root, "bin"));
  writeFileSync(join(root, "bin", "npm"), "#!/usr/bin/env node\nconsole.log(JSON.stringify([{ name: 'fixture' }]));\n", { mode: 0o755 });
  const env = { ...process.env, PATH: `${join(root, "bin")}:${process.env.PATH}` };
  want("pack report without size", spawnSync("node", [cli, root], { encoding: "utf8", env }).status, 2);
} finally {
  rmSync(root, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("package-size.test: PASS");
