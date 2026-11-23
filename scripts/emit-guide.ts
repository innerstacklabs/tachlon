// Copies the user guide into a package's build: `guide/*` to `dist-lib/guide/`. Run after the bundle, which empties its output folder. Exits 1 when `guide/` holds no file.
//   node scripts/emit-guide.ts <package folder>
import { cpSync, readdirSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
if (!dir) throw new Error("usage: node scripts/emit-guide.ts <package folder>");
if (readdirSync(join(dir, "guide")).length === 0) {
  console.error(`emit-guide: ${dir}/guide holds no file`);
  process.exit(1);
}
cpSync(join(dir, "guide"), join(dir, "dist-lib", "guide"), { recursive: true });
