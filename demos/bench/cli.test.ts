// The commands the Benchmark section of docs/MEASURING.md gives a reader (setup, run, table) each
// answer `--help` with their usage and exit 0, without doing any of their work. The commands are read
// from the doc's block, so a command renamed, added or dropped there fails here.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { dragProfile, measure, missingNeed } from "./profile.ts";
import { parseArgs } from "./profile-format.ts";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MARKER = "<!-- demos/bench/cli.test.ts runs every command in the next block with --help. -->";

/** The script of each command in the sh block after MARKER in docs/MEASURING.md. */
function documentedCommands(doc: string): string[] {
  const at = doc.indexOf(MARKER);
  assert.notEqual(at, -1, "docs/MEASURING.md has the marker before the three commands");
  const block = doc.slice(at + MARKER.length).match(/^\s*```sh\n([\s\S]*?)\n```/);
  assert.ok(block, "a ```sh block follows the marker");
  return block[1]
    .split("\n")
    .map((l) => l.replace(/#.*$/, "").trim())
    .filter((l) => l !== "")
    .map((l) => {
      const m = l.match(/^node (\S+\.ts)\b/);
      assert.ok(m, `every line of the block is a node command: ${JSON.stringify(l)}`);
      return m[1];
    });
}

const commands = documentedCommands(readFileSync(join(repo, "docs/MEASURING.md"), "utf8"));

test("the doc gives the three commands: setup, run, table", () => {
  assert.equal(commands.length, 3);
});

for (const command of commands) {
  test(`${command} --help exits 0 with its usage`, () => {
    assert.ok(existsSync(join(repo, command)), `${command} exists`);
    // An empty working directory, so a command that took --help for an argument would show it.
    const cwd = mkdtempSync(join(tmpdir(), "bench-cli-"));
    const r = spawnSync(process.execPath, [join(repo, command), "--help"], { cwd, encoding: "utf8", timeout: 30_000 });
    assert.equal(r.status, 0, `exit ${r.status}\n${r.stdout}${r.stderr}`);
    assert.match(r.stdout, /^usage: node /m);
    assert.deepEqual(readdirSync(cwd), [], "--help wrote nothing");
  });
}

// A detached unit has no cargo bin folder on its PATH and a fresh worktree has no node_modules: profile.ts says so in one line before it builds.
const empty = () => mkdtempSync(join(tmpdir(), "bench-need-"));

test("profile.ts without wasm-bindgen on PATH exits 2 with the fix in one line, before it builds", () => {
  const r = spawnSync(process.execPath, [join(repo, "demos/bench/profile.ts"), "--cell", "chain-10", "--holder", "x"], { cwd: empty(), env: { PATH: empty() }, encoding: "utf8", timeout: 30_000 });
  assert.equal(r.status, 2, `exit ${r.status}\n${r.stdout}${r.stderr}`);
  assert.match(r.stderr, /^profile: wasm-bindgen is not on PATH: run `export PATH=.*; nothing was built\n$/);
});

test("profile.ts names the one-line fix when @playwright/test does not resolve from the checkout", () => {
  const bin = empty();
  writeFileSync(join(bin, "wasm-bindgen"), "#!/bin/sh\necho wasm-bindgen 0\n", { mode: 0o755 });
  assert.equal(missingNeed({ PATH: bin }, empty()), "no @playwright/test: run pnpm install in apps/hosted or packages/gantt");
});

// `--drag` writes each run's frame-interval p95 per drag to the run folder; `--critical-only` skips the open measurement and the head drag.
const stub = () => {
  const urls: string[] = [];
  let url = "";
  const page = { goto: async (u: string) => void urls.push((url = u)), on() {}, off() {}, waitForFunction: async () => ({ jsonValue: async () => ({ drag: { p50: 5, p95: url.includes("dragTarget=head") ? 11 : 7 }, dragResponseMs: { p50: 1, p95: 2 } }) }) };
  const open = async () => ({ baseUrl: "http://mini/", browser: { contexts: () => [{ pages: () => [page] }] }, close: async () => {} }) as any;
  return { urls, open };
};
const dragArgs = (out: string, ...more: string[]) => parseArgs(["--cell", "tree4-50", "--holder", "x", "--drag", "--runs", "2", "--out", out, ...more])!;
const p95s = (out: string, k: number) => JSON.parse(readFileSync(join(out, "run", `run-${k}`, "drag.json"), "utf8"));

test("--drag leaves one frame-interval p95 per drag in each run folder", async () => {
  const out = empty();
  await dragProfile(dragArgs(out), [["run", "bundle"]], stub().open);
  for (const k of [1, 2]) assert.deepEqual(p95s(out, k), { frameIntervalP95Ms: { head: 11, critical: 7 } });
});

test("--critical-only measures no open and no head drag: the stubbed driver sees only the critical drags", async () => {
  const out = empty();
  const { urls, open } = stub();
  const { runs } = await measure(dragArgs(out, "--critical-only"), [["run", "bundle"]], open);
  assert.equal(runs, undefined);
  assert.deepEqual(urls.map((u) => u.match(/[^/]+\.html|dragTarget=\w+/g)?.join(" ")), ["index.html dragTarget=critical", "index.html dragTarget=critical"]);
  assert.deepEqual(p95s(out, 2), { frameIntervalP95Ms: { critical: 7 } });
});

test("--critical-only needs --drag", () => {
  assert.equal(parseArgs(["--cell", "tree4-50", "--critical-only"]), undefined);
});
