// Writes the folder a publish runs on: the package's packed files and a `package.json` without `scripts`, `devDependencies` or `packageManager`. Exit 2 on no pack.
//   node scripts/stage-package.ts <package folder> <new folder>
import { stagePackage } from "./lib/stage-package.ts";

const [src, dest] = process.argv.slice(2);
if (!src || !dest) {
  console.error("usage: node scripts/stage-package.ts <package folder> <new folder>");
  process.exit(2);
}
try {
  stagePackage(src, dest);
  console.log(`stage-package: ${src} -> ${dest}`);
} catch (e) {
  console.error(`stage-package: ${(e as Error).message}`);
  process.exit(2);
}
