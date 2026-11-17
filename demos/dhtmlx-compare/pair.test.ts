import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import * as p from "./pair.ts";

// `sysctl vm.swapusage` as macOS prints it.
const SWAPUSAGE = "vm.swapusage: total = 2048.00M  used = 1024.50M  free = 1023.50M  (encrypted)\n";

// `ps -axww -o rss=,comm=` as macOS prints it: RSS in kB, then the executable's full path.
const PS = [
  "  1024 /sbin/launchd",
  "205312 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  " 98304 /Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/156.0.8078.73/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)",
  "  4096 /Users/ci/.cargo/bin/cargo",
  "",
].join("\n");

test("a vm.swapusage sample parses to swap used in kB", () => {
  assert.equal(p.parseSwapUsage(SWAPUSAGE), 1049088);
  assert.equal(p.parseSwapUsage("total = 0.00M  used = 0.00M  free = 0.00M  (encrypted)"), 0);
  assert.equal(p.parseSwapUsage("total = 4.00G  used = 1.50G  free = 2.50G  (encrypted)"), 1572864);
  assert.equal(p.parseSwapUsage("total = 1024.00K  used = 512.00K  free = 512.00K"), 512);
  assert.throws(() => p.parseSwapUsage("vm.swapusage: nothing here"), /cannot read swap used/);
});

test("a decimal-comma swap figure is refused, never read as a smaller number", () => {
  for (const used of ["1024,50M", "1100,50M", "1.024,50M", "12.5.1M", ".5M"]) {
    assert.throws(() => p.parseSwapUsage(`total = 2048,00M  used = ${used}  free = 1023,50M  (encrypted)`), /cannot read swap used/, used);
  }
});

test("the macOS reader runs sysctl and ps in the C locale", () => {
  const envs: Record<string, Record<string, string>> = {};
  const run = (cmd: string, _args: string[], env: Record<string, string>): string => {
    envs[cmd] = env;
    return cmd === "ps" ? PS : SWAPUSAGE;
  };
  const r = p.macReader(run, () => [1, 1, 1]);
  r.swapKb();
  r.busy();
  assert.equal(envs.sysctl?.LC_ALL, "C");
  assert.equal(envs.ps?.LC_ALL, "C");
});

test("a process ps shows by its kernel name in parentheses is still busy", () => {
  const procs = p.parsePs("  4096 (cargo)\n  2048 (rustc)\n  1024 (launchd)\n");
  assert.deepEqual(procs.map((x) => x.name), ["cargo", "rustc", "launchd"]);
  const run = (cmd: string): string => (cmd === "ps" ? "  1024 /sbin/launchd\n  4096 (cargo)\n" : SWAPUSAGE);
  assert.equal(p.macReader(run, () => [0.5, 0.5, 0.5]).busy(), "cargo");
});

test("ps output parses to a name and RSS per process, names with spaces kept whole", () => {
  const procs = p.parsePs(PS);
  assert.deepEqual(
    procs.map((x) => [x.name, x.rssKb]),
    [
      ["launchd", 1024],
      ["Google Chrome", 205312],
      ["Google Chrome Helper (Renderer)", 98304],
      ["cargo", 4096],
    ],
  );
});

test("the busy test names a build process and passes everything else", () => {
  for (const name of ["cargo", "rustc", "xcodebuild"]) assert.equal(p.isBusyName(name), true, name);
  for (const name of ["launchd", "Google Chrome", "node", "cargo-watcher"]) assert.equal(p.isBusyName(name), false, name);
});

test("the macOS reader takes load1 from os.loadavg, swap from sysctl and Chrome RSS from ps", () => {
  const calls: string[] = [];
  const run = (cmd: string, args: string[]): string => {
    calls.push([cmd, ...args].join(" "));
    if (cmd === "sysctl") return SWAPUSAGE;
    if (cmd === "ps") return PS;
    throw new Error(`unexpected ${cmd}`);
  };
  const r = p.macReader(run, () => [2.5, 2.0, 1.5]);
  assert.equal(r.load1(), 2.5);
  assert.equal(r.swapKb(), 1049088);
  assert.equal(r.chromeRssKb(), 205312 + 98304);
  assert.equal(r.busy(), "cargo");
  assert.match(r.loadText(), /^loadavg: 2\.50 2\.00 1\.50/);
  assert.ok(calls.includes("sysctl vm.swapusage"), calls.join("; "));
});

test("the macOS reader is quiet when no build process runs", () => {
  const run = (cmd: string): string => (cmd === "ps" ? "  1024 /sbin/launchd\n" : SWAPUSAGE);
  assert.equal(p.macReader(run, () => [0.5, 0.5, 0.5]).busy(), null);
});

test("each platform gets its reader; others are refused", () => {
  assert.equal(typeof p.readerFor("linux").load1, "function");
  assert.equal(typeof p.readerFor("darwin").swapKb, "function");
  assert.throws(() => p.readerFor("win32"), /no reader for win32/);
});

test("ps text with no process row is a misparse that throws, never a box with nothing busy", () => {
  for (const text of ["", "\n", "ps: illegal option -- w\n"]) assert.throws(() => p.parsePs(text), /cannot read any process/, JSON.stringify(text));
  assert.throws(() => p.macReader(() => "", () => [0.5, 0.5, 0.5]).busy(), /cannot read any process/);
});

test("the Linux busy name is one word, so wait.log's busy= field does not split", () => {
  assert.equal(p.RUNNER_JOB, "gitlab-runner-job");
});

test("a reader that throws while the command runs stops the command and exits 74, not the command's 1", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pair-reader-"));
  const pidFile = join(dir, "pid");
  // Swap reads fine until the command is up, then throws, as a sysctl that printed nothing would.
  const swapKb = () => {
    if (existsSync(pidFile)) throw new Error("sysctl printed nothing");
    return 100;
  };
  const r: p.Reader = { load1: () => 0, swapKb, chromeRssKb: () => 0, busy: () => null, loadText: () => "" };
  const cmd = [process.execPath, "-e", `require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(process.pid)); setTimeout(() => {}, 60000)`];
  const t0 = Date.now();
  assert.equal(await p.runSide(r, join(dir, "side"), dir, cmd, 60, 1000, 50), p.EXIT_READER);
  assert.ok(Date.now() - t0 < 30_000, "runSide waited for the command to end by itself");
  const pid = Number(readFileSync(pidFile, "utf8"));
  assert.throws(() => process.kill(pid, 0), /ESRCH/, "the command still runs");
});

test("a command that ignores SIGTERM is killed after the kill wait, and the run still exits 74 in bounded time", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pair-sigterm-"));
  const pidFile = join(dir, "pid");
  const swapKb = () => {
    if (existsSync(pidFile)) throw new Error("sysctl printed nothing");
    return 100;
  };
  const r: p.Reader = { load1: () => 0, swapKb, chromeRssKb: () => 0, busy: () => null, loadText: () => "" };
  const cmd = [process.execPath, "-e", `process.on("SIGTERM", () => {}); require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(process.pid)); setTimeout(() => {}, 60000)`];
  const t0 = Date.now();
  assert.equal(await p.runSide(r, join(dir, "side"), dir, cmd, 60, 1000, 50, 300), p.EXIT_READER);
  assert.ok(Date.now() - t0 < 20_000, `took ${Date.now() - t0} ms: the run waited on a command that ignores SIGTERM`);
  assert.throws(() => process.kill(Number(readFileSync(pidFile, "utf8")), 0), /ESRCH/, "the command still runs");
});
