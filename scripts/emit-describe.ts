// Writes what `describe` prints into a package's build: `dist-lib/describe.json` (scripts/lib/describe-model.ts) and `dist-lib/fit.md` (a copy of src/cli/fit.md). Run after the bundle, which empties its output folder.
//   node scripts/emit-describe.ts <package folder>
import { copyFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { buildModel } from "./lib/describe-model.ts";

const dir = process.argv[2];
if (!dir) throw new Error("usage: node scripts/emit-describe.ts <package folder>");
writeFileSync(join(dir, "dist-lib", "describe.json"), `${JSON.stringify(buildModel(dir))}\n`);
copyFileSync(join(dir, "src", "cli", "fit.md"), join(dir, "dist-lib", "fit.md"));
