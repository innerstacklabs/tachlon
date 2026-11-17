// The workspace crates whose tests a run owes the `rust` job's test step (gantt#1456). `select` in `.github/workflows/ci.yml` calls
// `scripts/touched-crates.ts`, which calls this. Pure, so the table in `scripts/test-touched-crates.ts` is the whole rule.
//
// On the fast tier (a PR branch) a crate's tests run when the diff changes a file under that crate's folder, or under a crate it depends on
// (normal, dev or build dependency: a dependent's tests exercise it). A file outside every crate adds `app`, whose architecture tests read
// the web sources, the docs and every crate's sources. `EXTRA_READERS` names the folders another crate's tests read (the fixtures `import` reads, the token stylesheet `timeline` reads).
// A file that can change what any test does (the lock file, the toolchain, the workspace manifest, the nextest settings, the workflows), a
// folder under `crates/` that is no crate, a diff that could not be read or is empty, and every other tier run every crate: `rust` is `all`.
// fmt, clippy, doc and deny are not narrowed, only the tests.
import type { Tier } from "./ci-tier.ts";
import { WEB_ROOT } from "./web-root.ts";

export interface Crate {
  /** The package name, as `-p` takes it. */
  name: string;
  /** Its folder relative to the repo root, without a trailing slash. */
  dir: string;
  /** The workspace crates it depends on, of any kind. */
  deps: string[];
}

export interface CratePlan {
  /** all: the test step has no `-p`. some: it runs `packages`, in the order of the workspace. */
  rust: "all" | "some";
  packages: string[];
  /** One line for the log. */
  why: string;
}

/** Files at the repo root, or folders, that change what any crate's tests do. */
const EVERYTHING: Array<(file: string) => boolean> = [
  (f) => f === "Cargo.lock" || f === "Cargo.toml" || f.startsWith("rust-toolchain"),
  (f) => f.startsWith(".cargo/") || f.startsWith(".config/") || f.startsWith(".github/"),
];

/** The crate whose tests read files all over the repo (crates/app/tests/architecture). */
const REPO_READER = "app";

/** Folders and files read by the tests of a crate that is not their owner and not a dependent of it: a change there adds those crates. */
const EXTRA_READERS: Array<{ prefix: string; readers: string[] }> = [
  { prefix: `${WEB_ROOT}/tests/fixtures/`, readers: ["import"] },
  { prefix: "crates/app/tests/fixtures/", readers: ["import"] },
  { prefix: `${WEB_ROOT}/src/styles/tokens.css`, readers: ["timeline"] },
];

/** The crates of `cargo metadata --no-deps --format-version 1` output (only the fields read here). */
export function workspaceCrates(metadata: {
  workspace_root: string;
  packages: Array<{ name: string; manifest_path: string; dependencies: Array<{ name: string; path?: string | null }> }>;
}): Crate[] {
  const dirOf = (manifest: string): string => manifest.slice(metadata.workspace_root.length + 1, manifest.lastIndexOf("/"));
  const names = new Map(metadata.packages.map((p) => [dirOf(p.manifest_path), p.name]));
  return metadata.packages.map((p) => ({
    name: p.name,
    dir: dirOf(p.manifest_path),
    deps: [...new Set(p.dependencies.flatMap((d) => (d.path ? [names.get(d.path.slice(metadata.workspace_root.length + 1))] : [])))].filter((n): n is string => !!n && n !== p.name),
  }));
}

/** What the test step runs on a run: `changed` is the diff against main (null: it could not be read). */
export function cratesPlan(tier: Tier, changed: string[] | null, crates: Crate[]): CratePlan {
  const everything = (why: string): CratePlan => ({ rust: "all", packages: [], why });
  if (tier !== "fast") return everything(`the ${tier} tier runs every crate`);
  if (changed === null) return everything("the diff against main could not be read");
  if (changed.length === 0) return everything("no changed files against main to judge");
  if (crates.some((c) => !/^[\w-]+$/.test(c.name))) return everything("a package name is not safe to pass to the shell");
  const touched = new Set<string>();
  for (const file of changed) {
    if (EVERYTHING.some((rule) => rule(file))) return everything(`${file} can change what any test does`);
    const owner = crates.find((c) => file.startsWith(`${c.dir}/`));
    if (!owner && file.startsWith("crates/")) return everything(`${file} is under crates/ and in no crate of the workspace`);
    if (owner) touched.add(owner.name);
    else touched.add(REPO_READER);
    for (const extra of EXTRA_READERS.filter((e) => file.startsWith(e.prefix))) for (const r of extra.readers) touched.add(r);
  }
  if (touched.has(REPO_READER) && !crates.some((c) => c.name === REPO_READER)) return everything(`the workspace has no ${REPO_READER} crate`);
  // The reverse-dependency closure: add every crate that depends on one already in, until nothing is added.
  for (let grew = true; grew; ) {
    grew = false;
    for (const c of crates.filter((c) => !touched.has(c.name) && c.deps.some((d) => touched.has(d)))) {
      touched.add(c.name);
      grew = true;
    }
  }
  const packages = crates.map((c) => c.name).filter((n) => touched.has(n));
  const skipped = crates.length - packages.length;
  return { rust: "some", packages, why: `the diff reaches ${packages.join(", ")}${skipped > 0 ? `; ${skipped} crate(s) skipped` : ""}` };
}
