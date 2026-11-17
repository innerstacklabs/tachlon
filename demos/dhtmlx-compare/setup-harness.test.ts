import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  rewriteImports,
  rewritePackageJson,
  assertOutsideWorkTree,
  HARNESS_COMMIT,
} from "./setup-harness.ts";

const here = dirname(fileURLToPath(import.meta.url));

const FIXTURE = [
  'import { gantt } from "@dhx/trial-gantt";',
  'import "@dhx/trial-gantt/codebase/dhtmlxgantt.css";',
  "export const x = 1;",
].join("\n");

test("a three-line import fixture is rewritten to dhtmlx-gantt", () => {
  const out = rewriteImports(FIXTURE);
  assert.equal(
    out,
    [
      'import { gantt } from "dhtmlx-gantt";',
      'import "dhtmlx-gantt/codebase/dhtmlxgantt.css";',
      "export const x = 1;",
    ].join("\n"),
  );
  assert.ok(!out.includes("trial-gantt"));
});

test("the dependency is swapped to the Community package and its version kept", () => {
  const out = JSON.parse(
    rewritePackageJson(JSON.stringify({ dependencies: { "@dhx/trial-gantt": "10.0.3" } })),
  );
  assert.deepEqual(out.dependencies, { "dhtmlx-gantt": "10.0.3" });
});

test("the pinned commit is a full SHA", () => {
  assert.match(HARNESS_COMMIT.sha, /^[0-9a-f]{40}$/);
  assert.match(HARNESS_COMMIT.dateRead, /^\d{4}-\d{2}-\d{2}$/);
});

test("a scratch path inside a work tree throws", () => {
  const repo = mkdtempSync(join(tmpdir(), "swh-repo-"));
  try {
    execFileSync("git", ["init", "-q", repo]);
    const inside = join(repo, "sub", "scratch");
    mkdirSync(join(repo, "sub"), { recursive: true });
    assert.throws(() => assertOutsideWorkTree(inside), /work tree/);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("a scratch path outside any work tree is accepted", () => {
  const dir = mkdtempSync(join(tmpdir(), "swh-ok-"));
  try {
    assertOutsideWorkTree(join(dir, "scratch"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the CLI exits non-zero for a scratch path inside a work tree", () => {
  const repo = mkdtempSync(join(tmpdir(), "swh-cli-"));
  try {
    execFileSync("git", ["init", "-q", repo]);
    writeFileSync(join(repo, "f"), "x");
    const r = spawnSync(
      process.execPath,
      [join(here, "setup-harness.ts"), join(repo, "scratch")],
      { encoding: "utf8" },
    );
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /work tree/);
    assert.equal(readFileSync(join(repo, "f"), "utf8"), "x");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});
