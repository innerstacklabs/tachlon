// What a side's build is, written beside each run as side.json, so the table never combines two
// builds of a side, or two versions of the harness, into one row.
//
// `bundleSha256` is one SHA-256 over every file in the side's bundle directory, in sorted path
// order (each path, then its bytes; a symlink by its target text, never followed). `probe` and
// `path` are the side's sides.json entries. `harness` is the commit of this repository that ran
// the pairs, with `-dirty` when demos/ has changes not committed, or "unknown" without git.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstatSync, readdirSync, readFileSync, readlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export interface SideBuild {
  name: string;
  probe: string;
  path: string;
  bundleSha256: string;
  harness: string;
}

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/** The SHA-256 of a directory's files: sorted relative paths, each followed by its bytes. */
export function bundleSha256(dir: string): string {
  const h = createHash("sha256");
  const walk = (rel: string) => {
    for (const name of readdirSync(join(dir, rel)).sort()) {
      const r = rel === "" ? name : `${rel}/${name}`;
      const st = lstatSync(join(dir, r));
      if (st.isDirectory()) walk(r);
      else {
        h.update(`${r}\0`);
        h.update(st.isSymbolicLink() ? `-> ${readlinkSync(join(dir, r))}` : readFileSync(join(dir, r)));
        h.update("\0");
      }
    }
  };
  walk("");
  return h.digest("hex");
}

/** This repository's commit, `-dirty` when demos/ has uncommitted changes; "unknown" without git. */
export function harnessVersion(): string {
  try {
    const head = execFileSync("git", ["-C", repo, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
    const dirty = execFileSync("git", ["-C", repo, "status", "--porcelain", "--", "demos"], { encoding: "utf8" }).trim() !== "";
    return dirty ? `${head}-dirty` : head;
  } catch {
    return "unknown";
  }
}

/** A side's build: its sides.json entry, its bundle's hash and the harness version. */
export function sideBuild(s: { name: string; bundle: string; probe: string; path: string }, harness: string): SideBuild {
  return { name: s.name, probe: s.probe, path: s.path, bundleSha256: bundleSha256(s.bundle), harness };
}
