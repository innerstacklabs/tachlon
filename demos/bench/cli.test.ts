// The commands the Benchmark section of docs/MEASURING.md gives a reader (setup, run, table) each
// answer `--help` with their usage and exit 0, without doing any of their work. The commands are read
// from the doc's block, so a command renamed, added or dropped there fails here.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

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
