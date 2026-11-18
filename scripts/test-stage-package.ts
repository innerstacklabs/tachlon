// Checks the staged publish folder (see scripts/lib/stage-package.ts) on a temporary package that holds build settings. Uses the real `npm pack --dry-run`, which runs offline.
//   node scripts/test-stage-package.ts
// Exits 1, one FAIL line per problem.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { stagePackage } from "./lib/stage-package.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-stage-package: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

const root = mkdtempSync(join(tmpdir(), "test-stage-package-"));
try {
  // A package that holds every file the real pack check expects, one source file the tarball leaves out, and the three private keys beside the keys a consumer needs.
  const src = join(root, "src-pkg");
  const files = ["README.md", "LICENSE", "AGENTS.md", "src/secret.ts", "dist-lib/a.js", "dist-lib/app_bg.wasm", "dist-lib/engine-size.json", "dist-lib/schema.json", "dist-lib/describe.json", "dist-lib/fit.md", "dist-lib/NOTICE.md", "dist-lib/OFL.txt"];
  for (const f of files) {
    mkdirSync(dirname(join(src, f)), { recursive: true });
    writeFileSync(join(src, f), f === "dist-lib/a.js" ? "#!/usr/bin/env node\n" : "x\n");
  }
  const manifest = { name: "p", version: "1.0.0", bin: "./dist-lib/a.js", exports: { ".": "./dist-lib/a.js" }, files: ["dist-lib", "AGENTS.md"], peerDependencies: { react: ">=18" }, scripts: { build: "x" }, devDependencies: { y: "1" }, packageManager: "pnpm@1" };
  writeFileSync(join(src, "package.json"), JSON.stringify(manifest));
  const dest = join(root, "staged");
  stagePackage(src, dest);

  const { scripts, devDependencies, packageManager, ...kept } = manifest;
  check("staged manifest drops the build settings and keeps the rest", JSON.parse(readFileSync(join(dest, "package.json"), "utf8")), kept);
  check("staged folder holds the packed files and not the source", [files.filter((f) => existsSync(join(dest, f))).join(" ")], [files.filter((f) => f !== "src/secret.ts").join(" ")]);
  const packCheck = (dir: string) => spawnSync(process.execPath, [fileURLToPath(new URL("./pack-check.ts", import.meta.url)), dir, "--published"], { encoding: "utf8" });
  check("source fails the published check", packCheck(src).status, 1);
  check("staged passes the published check", packCheck(dest).status, 0);
} finally {
  rmSync(root, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("test-stage-package: ok");
