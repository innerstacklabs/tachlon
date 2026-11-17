// The profile command's stall guard and progress lines (docs/PROFILING.md, "Options"): a timer that restarts on each finished session, and one line to stderr for each.
import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { baseline, interleaved } from "../../../demos/bench/profile.ts";
import { type Args, watchdog } from "../../../demos/bench/profile-format.ts";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

test("a watchdog with no mark calls onStall once, with the limit it ran out at", async () => {
  const calls: [string, number][] = [];
  watchdog(40, (last, limit) => calls.push([last, limit]));
  await wait(200);
  assert.deepEqual(calls, [["", 40]]);
});

test("marks inside the limit keep the watchdog quiet, and the text of the last one is what a stall reports", async () => {
  const calls: [string, number][] = [];
  const dog = watchdog(40, (last, limit) => calls.push([last, limit]));
  for (let i = 1; i <= 6; i++) {
    dog.mark(`session ${i}`);
    await wait(15);
  }
  assert.deepEqual(calls, []);
  await wait(200);
  assert.deepEqual(calls, [["session 6", 40]]);
});

test("a mark can set a shorter limit for the wait that follows it", async () => {
  const calls: [string, number][] = [];
  const dog = watchdog(10_000, (last, limit) => calls.push([last, limit]));
  dog.mark("last session", 40);
  await wait(200);
  assert.deepEqual(calls, [["last session", 40]]);
});

test("stop before the limit means onStall is never called", async () => {
  let called = 0;
  const dog = watchdog(40, () => called++);
  dog.stop();
  await wait(200);
  assert.equal(called, 0);
});

test("each finished session prints a line to stderr that names its side and its number", async (t) => {
  const lines: string[] = [];
  t.mock.method(console, "error", (line: string) => lines.push(line));
  const session = { browser: undefined, baseUrl: "", footprintKb: async () => new Map(), close: async () => undefined };
  const a = { runs: 2, host: "mini", chrome: "chrome" } as unknown as Args;
  await interleaved(a, [["before", "b"], ["after", "a"]], async () => session, async () => 1);
  assert.deepEqual(lines, ["before session 1/2 done", "after session 1/2 done", "before session 2/2 done", "after session 2/2 done"]);
});

test("the baseline is the commit sha a ref names at that moment (a tag giving its commit), and an unknown ref is refused", () => {
  const dir = mkdtempSync(join(tmpdir(), "baseline-"));
  const git = (...a: string[]) => execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@t", ...a], { encoding: "utf8" }).trim();
  git("init", "-q");
  git("commit", "-q", "--allow-empty", "-m", "first");
  git("tag", "-a", "-m", "release", "v1");
  const first = git("rev-parse", "HEAD");
  assert.deepEqual([baseline("HEAD", dir), baseline("v1", dir)], [first, first]);
  git("commit", "-q", "--allow-empty", "-m", "second");
  assert.notEqual(baseline("HEAD", dir), first);
  assert.equal(baseline(first, dir), first);
  assert.throws(() => baseline("nope", dir));
});
