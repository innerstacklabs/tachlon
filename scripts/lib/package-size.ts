// The packed tarball's byte budget (tl#1091). `npm pack --dry-run` reports the tarball's size without writing it.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// The bytes of the tarball `npm pack` would write for the package in `dir`. Throws when the pack report has no size.
export function tarballBytes(dir: string): number {
  const [packed] = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
  if (typeof packed?.size !== "number") throw new Error(`npm pack --dry-run --json in ${dir} reported no "size" number`);
  return packed.size;
}

// The budget in `<dir>/size-budget.json`: `{ "bytes": n, ... }`. Throws when the file is missing or has no number.
export function budgetBytes(dir: string): number {
  const { bytes } = JSON.parse(readFileSync(join(dir, "size-budget.json"), "utf8"));
  if (typeof bytes !== "number") throw new Error(`${dir}/size-budget.json has no "bytes" number`);
  return bytes;
}
