// Records the byte length of a package's built engine (`dist-lib/app_bg.wasm`) in `dist-lib/engine-size.json`, which the tarball ships.
// The package's `doctor` command compares an installed engine to that recorded value, so a cut-short file is caught even when the command that runs is the installed copy itself.
// Run after the bundle, which empties its output folder.
//   node scripts/emit-engine-size.ts <package folder>
import { statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/emit-engine-size.ts <package folder>");
  process.exit(2);
}
writeFileSync(join(dir, "dist-lib", "engine-size.json"), `${JSON.stringify({ bytes: statSync(join(dir, "dist-lib", "app_bg.wasm")).size })}\n`);
