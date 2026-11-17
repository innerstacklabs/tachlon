// Prints the crates whose tests a run owes, as lines ready to append to $GITHUB_OUTPUT. Run at the repo root, with origin/main fetched
// and `cargo` on the PATH:
//   node scripts/touched-crates.ts "$TIER" >> "$GITHUB_OUTPUT"
// The rule is in scripts/lib/touched-crates.ts. Needs Node 22.18 or later (type stripping). It prints:
//   rust=all|some            whether the test step runs the whole workspace or only some crates
//   rust_packages=<-p list>  for some, `-p <crate>` per crate, ready to put after `cargo nextest run`; empty for all
// A diff or a workspace it cannot read prints rust=all, never fewer. Exits 2 and prints nothing to stdout when the tier is missing or unknown.
import { spawnSync } from "node:child_process";
import { cratesPlan, workspaceCrates, type Crate } from "./lib/touched-crates.ts";
import type { Tier } from "./lib/ci-tier.ts";

const tier = process.argv[2] as Tier | undefined;
if (!tier || !["fast", "full", "nightly", "deploy"].includes(tier)) {
  console.error("touched-crates: usage: node scripts/touched-crates.ts <fast|full|nightly|deploy>");
  process.exit(2);
}
let crates: Crate[] = [];
let changed: string[] | null = null;
if (tier === "fast") {
  // --no-renames: a rename counts both of its paths, so a file moved out of a folder a crate reads still reaches that crate.
  const diff = spawnSync("git", ["diff", "--name-only", "--no-renames", "origin/main...HEAD"], { encoding: "utf8" });
  const meta = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], { encoding: "utf8", maxBuffer: 1 << 26 });
  try {
    if (meta.status !== 0) throw new Error(`cargo metadata exited ${meta.status}: ${meta.stderr.trim()}`);
    crates = workspaceCrates(JSON.parse(meta.stdout));
    changed = diff.status === 0 ? diff.stdout.split("\n").filter(Boolean) : null;
  } catch (error) {
    console.error(`touched-crates: ${error}`);
  }
}
const plan = cratesPlan(tier, changed, crates);
console.error(`touched-crates: rust=${plan.rust} (${plan.why})`);
console.log(`rust=${plan.rust}`);
console.log(`rust_packages=${plan.packages.map((p) => `-p ${p}`).join(" ")}`);
