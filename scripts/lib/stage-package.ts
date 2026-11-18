// The folder a publish runs on (tl#1560): the files the package would pack, with a `package.json` that holds none of the project's build settings.
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { PRIVATE_KEYS, packedFiles } from "./pack-check.ts";

// `pkg` without `scripts`, `devDependencies` and `packageManager`; every other key stays.
export function publishManifest(pkg: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(pkg).filter(([k]) => !PRIVATE_KEYS.includes(k)));
}

// Writes into `dest` the files `src`'s tarball would hold, its `package.json` through `publishManifest`.
export function stagePackage(src: string, dest: string): void {
  for (const f of [...packedFiles(src)].filter((p) => p !== "package.json")) {
    mkdirSync(dirname(join(dest, f)), { recursive: true });
    copyFileSync(join(src, f), join(dest, f));
  }
  mkdirSync(dest, { recursive: true });
  writeFileSync(join(dest, "package.json"), `${JSON.stringify(publishManifest(JSON.parse(readFileSync(join(src, "package.json"), "utf8"))), null, 2)}\n`);
}
