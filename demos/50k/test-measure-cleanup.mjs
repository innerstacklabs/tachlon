// Checks measure.mjs parsing and cleanup; run with: node demos/50k/test-measure-cleanup.mjs
// Needs packages/gantt/dist (scripts/build-web.sh) and Playwright Chromium
// (pnpm exec playwright install chromium). It signals only processes it starts.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync } from "node:fs";
import { createServer, createConnection } from "node:net";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { newestChromium, parseParam, stopGroup } from "./measure.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, "../..");
const MEASURE = resolve(process.env.GS_MEASURE ?? join(HERE, "measure.mjs"));
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

if (process.env.GS_PIDNS !== "1") {
  const bwrapArgs = ["--dev-bind", "/", "/", "--unshare-pid", "--proc", "/proc", "--die-with-parent"];
  const probe = spawnSync("bwrap", [...bwrapArgs, "true"], { encoding: "utf8" });
  if (probe.error?.code === "ENOENT") {
    console.error("cannot run: bwrap is missing; install bubblewrap and try again");
    process.exit(2);
  }
  if (probe.status !== 0) {
    console.error(`cannot run bwrap: ${probe.stderr.trim() || `exit ${probe.status}`}; install/configure bubblewrap and enable user namespaces`);
    process.exit(2);
  }
  const run = spawnSync("bwrap", [...bwrapArgs, process.execPath, fileURLToPath(import.meta.url)], {
    env: { ...process.env, GS_PIDNS: "1" }, stdio: "inherit",
  });
  if (run.error) console.error(`cannot run bwrap: ${run.error.message}`);
  process.exit(run.status ?? 2);
}

if (!existsSync(join(ROOT, "packages", "gantt", "dist"))) {
  console.error("cannot run: packages/gantt/dist is missing; build it with scripts/build-web.sh");
  process.exit(2);
}
const chromiumBase = join(homedir(), ".cache", "ms-playwright");
let chromium;
try {
  const dir = newestChromium(readdirSync(chromiumBase));
  chromium = dir && join(chromiumBase, dir, "chrome-linux64", "chrome");
} catch {}
if (!chromium || !existsSync(chromium)) {
  console.error("cannot run: Playwright Chromium is missing; run pnpm exec playwright install chromium");
  process.exit(2);
}

function procs() {
  const result = [];
  for (const name of readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    try {
      const stat = readFileSync(`/proc/${name}/stat`, "utf8");
      const fields = stat.slice(stat.lastIndexOf(")") + 2).trim().split(/\s+/);
      const argv = readFileSync(`/proc/${name}/cmdline`).toString().split("\0");
      if (argv.at(-1) === "") argv.pop();
      result.push({ pid: Number(name), ppid: Number(fields[1]), pgrp: Number(fields[2]), argv });
    } catch (error) {
      if (error?.code !== "ENOENT" && error?.code !== "ESRCH") throw error;
    }
  }
  return result;
}

function portOpen(port) {
  return new Promise((resolvePort, reject) => {
    const socket = createConnection({ host: "127.0.0.1", port });
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolvePort(value);
    };
    socket.setTimeout(300, () => finish(false));
    socket.once("connect", () => finish(true));
    socket.once("error", (error) => {
      if (error.code === "ECONNREFUSED") finish(false);
      else if (!settled) reject(error);
    });
  });
}

function freePort() {
  return new Promise((resolvePort, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolvePort(port));
    });
  });
}

function same(actual, expected) {
  return actual.length === expected.length && actual.every((value, i) => value === expected[i]);
}

function pnpmPreview(argv, wantedPort) {
  for (const start of [1, 2]) {
    if (argv.length !== start + 6 || !argv.some((value) => value.endsWith("pnpm"))) continue;
    const tail = argv.slice(start);
    if (tail[0] !== "preview" || tail[1] !== "--host" || tail[2] !== "127.0.0.1" ||
        tail[3] !== "--port" || !/^\d+$/.test(tail[4]) || tail[5] !== "--strictPort") continue;
    if (wantedPort === undefined || tail[4] === String(wantedPort)) return Number(tail[4]);
  }
}

function previewForms(port) {
  const args = ["preview", "--host", "127.0.0.1", "--port", String(port), "--strictPort"];
  return [
    ["sh", "-c", `vite ${args.join(" ")}`],
    ["node", `${ROOT}/packages/gantt/node_modules/.bin/../vite/bin/vite.js`, ...args],
  ];
}

function isPreviewProcess(proc, port) {
  return pnpmPreview(proc.argv, port) !== undefined || previewForms(port).some((form) => same(proc.argv, form));
}

async function waitFor(predicate, ms, message) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await sleep(200);
  }
  throw new Error(typeof message === "function" ? message() : message);
}

let passed = 0;
let failed = 0;
async function check(name, action) {
  try {
    await action();
    passed++;
    console.log(`ok - ${name}`);
  } catch (error) {
    failed++;
    console.log(`not ok - ${name}: ${error?.message ?? error}`);
  }
}

await check("parseParam", () => {
  assert.deepEqual(parseParam("n=5000"), ["n", "5000"]);
  assert.deepEqual(parseParam("label=a=b=c"), ["label", "a=b=c"]);
  assert.deepEqual(parseParam("k"), ["k", "1"]);
  assert.deepEqual(parseParam("k="), ["k", ""]);
});

await check("newestChromium", () => {
  assert.equal(newestChromium(["chromium-999", "chromium-1223", "chromium_headless_shell-1223"]), "chromium-1223");
  assert.equal(newestChromium(["chromium-9", "chromium-10"]), "chromium-10");
  assert.equal(newestChromium(["chromium_headless_shell-1"]), undefined);
  assert.equal(newestChromium([]), undefined);
});

await check("stopGroup uses SIGKILL fallback", async () => {
  const port = await freePort();
  const stub = "require('node:http').createServer((q,s)=>s.end('ok')).listen(Number(process.argv[1]),'127.0.0.1');process.on('SIGTERM',()=>{})";
  const child = spawn(process.execPath, ["-e", stub, String(port)], { detached: true, stdio: "ignore" });
  try {
    await waitFor(() => portOpen(port), 5000, "stub server did not start within 5 seconds");
    const started = Date.now();
    await stopGroup(child, `http://127.0.0.1:${port}/`, { graceMs: 1000 });
    assert.ok(Date.now() - started >= 900, "stopGroup returned before its SIGKILL grace period");
    assert.equal(await portOpen(port), false, "stub port is still open");
    assert.equal(procs().some((proc) => proc.pgrp === child.pid), false, "stub process group is still present");
  } finally {
    if (procs().some((proc) => proc.pgrp === child.pid)) {
      try { process.kill(-child.pid, "SIGKILL"); } catch (error) { if (error.code !== "ESRCH") throw error; }
    }
  }
});

async function runSignalCase(signal, expectedStatus, options = {}) {
  const out = mkdtempSync(join(tmpdir(), "gs-measure-cleanup-"));
  const measure = spawn(process.execPath, [MEASURE, "--headless", "--out", out, "--param", "n=1000"], {
    env: { ...process.env, ...options.env },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  let exitResult;
  let spawnError;
  let group;
  let port;
  measure.stdout.on("data", (chunk) => { stdout += chunk; });
  measure.stderr.on("data", (chunk) => { stderr += chunk; });
  measure.once("error", (error) => { spawnError = error; });
  const exited = new Promise((done) => measure.once("exit", (code, signalName) => {
    exitResult = { code, signal: signalName };
    done(exitResult);
  }));
  const tail = () => `stdout:\n${stdout.slice(-2000)}\nstderr:\n${stderr.slice(-2000)}`;
  try {
    await waitFor(async () => {
      if (spawnError) throw spawnError;
      if (exitResult) throw new Error(`measure exited before it was ready (${exitResult.code ?? exitResult.signal})\n${tail()}`);
      const all = procs();
      const leaders = all.filter((proc) => proc.ppid === measure.pid && pnpmPreview(proc.argv) !== undefined);
      for (const leader of leaders) {
        group = leader.pid;
        port = pnpmPreview(leader.argv);
        const members = all.filter((proc) => proc.pgrp === group);
        const [shell, vite] = previewForms(port);
        if (members.some((proc) => proc.pid === group) && members.some((proc) => same(proc.argv, shell)) &&
            members.some((proc) => same(proc.argv, vite)) && /measuring on/.test(stdout) && await portOpen(port)) return true;
      }
      return false;
    }, 90_000, () => `measure was not ready within 90 seconds\n${tail()}`);

    assert.equal(measure.kill(signal), true, `could not send ${signal} to measure`);
    if (options.secondSignal) {
      await sleep(500);
      const secondSignalAt = Date.now();
      assert.equal(measure.kill(options.secondSignal), true, `could not send second ${options.secondSignal} to measure`);
      await waitFor(() => exitResult !== undefined, 5000, () => `measure did not exit within 5 seconds of the second signal\n${tail()}`);
      assert.ok(Date.now() - secondSignalAt < 5000, "measure did not exit within 5 seconds of the second signal");
    } else {
      await waitFor(() => exitResult !== undefined, 30_000, () => `measure did not exit within 30 seconds\n${tail()}`);
    }
    const result = await exited;
    assert.equal(result.code, expectedStatus, `exit was code ${result.code}, signal ${result.signal}`);
    assert.equal(await portOpen(port), false, `preview port ${port} is still open`);
    const remaining = procs();
    assert.equal(remaining.some((proc) => proc.pgrp === group), false, `preview group ${group} is still present`);
    assert.equal(remaining.some((proc) => isPreviewProcess(proc, port)), false, `a preview process for port ${port} remains`);
  } finally {
    if (!group && measure.pid) {
      const leader = procs().find((proc) => proc.ppid === measure.pid && pnpmPreview(proc.argv) !== undefined);
      group = leader?.pid;
    }
    if (!exitResult && measure.pid) measure.kill("SIGKILL");
    if (group && procs().some((proc) => proc.pgrp === group)) {
      try { process.kill(-group, "SIGKILL"); } catch (error) { if (error.code !== "ESRCH") throw error; }
    }
    rmSync(out, { recursive: true, force: true });
  }
}

await check("SIGINT cleanup", () => runSignalCase("SIGINT", 130));
await check("SIGTERM cleanup", () => runSignalCase("SIGTERM", 143));
await check("SIGINT cleanup with delayed handler", () => runSignalCase("SIGINT", 130, {
  env: { GS_MEASURE_SIGNAL_DELAY_MS: "1500" },
}));
await check("SIGTERM cleanup with delayed handler", () => runSignalCase("SIGTERM", 143, {
  env: { GS_MEASURE_SIGNAL_DELAY_MS: "1500" },
}));
await check("second signal exits at once", () => runSignalCase("SIGINT", 130, {
  env: { GS_MEASURE_SIGNAL_DELAY_MS: "20000" }, secondSignal: "SIGINT",
}));

await check("runs through a symlink", () => {
  const temp = mkdtempSync(join(tmpdir(), "gs-measure-symlink-"));
  try {
    symlinkSync(ROOT, join(temp, "repo"));
    const result = spawnSync(process.execPath, [
      join(temp, "repo", "demos", "50k", "measure.mjs"),
      "--url", "http://127.0.0.1:1/", "--chrome", "/nonexistent",
    ], { encoding: "utf8", timeout: 30_000 });
    assert.equal(typeof result.status, "number", `symlinked measure did not return an exit status: ${result.error?.message ?? "unknown error"}`);
    assert.notEqual(result.status, 0, "symlinked measure exited successfully without running main");
    assert.notEqual(result.stderr, "", "symlinked measure produced no stderr");
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
});

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
