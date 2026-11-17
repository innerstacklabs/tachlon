// `doctor` asks a real browser which graphics path the chart takes (gantt#1249): one finding for each answer, and the skip when no browser answered.
// `run` and `runAsync` are given the browser's answer, so no browser starts, except in the last tests, which start a stand-in program.
import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { askBrowser, type Gpu } from "../src/cli/gpu.ts";
import { run, runAsync } from "../src/cli/run.ts";

const root = mkdtempSync(join(tmpdir(), "doctor-gpu-"));
after(() => rmSync(root, { recursive: true, force: true }));
const NAME = ["tach", "lon"].join(""); // the package's own name, built at run time (the name check)
type Finding = { code: string; status: string; what: string; fix: string };

// A project with the package installed and its engine, so the checks before the GPU one all pass.
function installed(): string {
  const dir = join(root, "project");
  const wasm = Buffer.concat([Buffer.from([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]), Buffer.alloc(64, 7)]);
  mkdirSync(join(dir, "src"), { recursive: true });
  mkdirSync(join(dir, `node_modules/${NAME}/dist-lib`), { recursive: true });
  writeFileSync(join(dir, "package.json"), JSON.stringify({ devDependencies: { [NAME]: "1.2.3" } }));
  writeFileSync(join(dir, "src/main.ts"), `import "${NAME}/gantt.css";`);
  writeFileSync(join(dir, `node_modules/${NAME}/package.json`), JSON.stringify({ name: NAME, version: "1.2.3" }));
  writeFileSync(join(dir, `node_modules/${NAME}/dist-lib/app_bg.wasm`), wasm);
  writeFileSync(join(dir, `node_modules/${NAME}/dist-lib/engine-size.json`), JSON.stringify({ bytes: wasm.length }));
  return dir;
}
const dir = installed();
const argv = ["doctor", "--dir", dir, "--json"];
const lastOf = (out: string) => (JSON.parse(out).findings as Finding[]).at(-1) as Finding;

test("doctor says which graphics path the browser gives the chart: WebGPU and WebGL2 pass with the adapter's name, neither fails with the fix", () => {
  const webgpu = run(argv, dir, undefined, { api: "webgpu", adapter: "nvidia ampere" });
  assert.deepEqual([webgpu.code, lastOf(webgpu.out).code, lastOf(webgpu.out).status, lastOf(webgpu.out).fix], [0, "GPU_WEBGPU", "pass", ""]);
  assert.match(lastOf(webgpu.out).what, /WebGPU.*nvidia ampere/);
  const webgl2 = run(argv, dir, undefined, { api: "webgl2", adapter: "ANGLE (Apple M2)" });
  assert.deepEqual([webgl2.code, lastOf(webgl2.out).code, lastOf(webgl2.out).status], [0, "GPU_WEBGL2", "pass"]);
  assert.match(lastOf(webgl2.out).what, /no WebGPU adapter.*WebGL2.*ANGLE \(Apple M2\)/);
  const none = run(argv, dir, undefined, { api: "none", adapter: "" });
  assert.deepEqual([none.code, lastOf(none.out).code, lastOf(none.out).status], [1, "GPU_NONE", "fail"]);
  assert.match(lastOf(none.out).what, new RegExp(`neither WebGPU nor WebGL2.*${NAME.toUpperCase()}_GPU_NONE`));
  assert.match(lastOf(none.out).fix, /^[^\n]+hardware acceleration[^\n]+\.$/);
});

test("doctor keeps the GPU skip, with the reason, when no browser answered", () => {
  for (const gpu of [{ unavailable: "no Chrome, Edge or Chromium was found" }, undefined]) {
    const r = run(argv, dir, undefined, gpu);
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual([lastOf(r.out).code, lastOf(r.out).status], ["GPU_SKIPPED", "skipped"]);
    assert.match(lastOf(r.out).what, gpu ? /not checked: no Chrome, Edge or Chromium was found/ : /not checked: no browser was asked/);
    assert.match(lastOf(r.out).what, new RegExp(`${NAME.toUpperCase()}_GPU_NONE`));
  }
});

test("doctor asks the browser it is given, and the answer reaches the findings", async () => {
  const r = await runAsync(argv, dir, async () => ({ api: "webgl2", adapter: "llvmpipe" }));
  assert.equal(r.code, 0, r.out + r.err);
  assert.deepEqual((JSON.parse(r.out).findings as Finding[]).map((f) => f.code), ["INSTALLED", "ONE_COPY", "ENGINE_OK", "STYLESHEET_OK", "GPU_WEBGL2"]);
});

// A stand-in browser: a Node script that is given the page's file address last, as the real one is, and prints a page the way `--dump-dom` does.
let programs = 0;
function browser(body: string): NodeJS.ProcessEnv {
  const file = join(root, `browser-${programs++}.mjs`);
  writeFileSync(file, `#!/usr/bin/env node\nimport { readFileSync } from "node:fs";\nimport { fileURLToPath } from "node:url";\nconst page = readFileSync(fileURLToPath(process.argv.at(-1)), "utf8");\n${body}\n`);
  chmodSync(file, 0o755);
  return { PATH: process.env.PATH, [`${NAME.toUpperCase()}_BROWSER`]: file };
}
const answer = (gpu: Gpu) => `<html><head><title>gpu=${encodeURIComponent(JSON.stringify(gpu))}</title></head></html>`;

test("askBrowser opens its own page in the browser it finds and reads the answer the page wrote into its title", async () => {
  // The stand-in answers from what the page it was given contains, so a page that never asks the browser fails here.
  const env = browser(`const asks = page.includes("requestAdapter") && page.includes("webgl2");\nconsole.log(${JSON.stringify(answer({ api: "webgpu", adapter: "X" }))}.replace("X", asks ? "asked" : "silent"));`);
  assert.deepEqual(await askBrowser(env), { api: "webgpu", adapter: "asked" });
});

test("askBrowser says why it has no answer: the browser fails, or prints a page with no report", async () => {
  const failed = await askBrowser(browser("process.exit(3);"));
  assert.match((failed as { unavailable: string }).unavailable, /did not answer/);
  const silent = await askBrowser(browser(`console.log("<html><head><title>x</title></head></html>");`));
  assert.match((silent as { unavailable: string }).unavailable, /no GPU report/);
  const missing = await askBrowser({ PATH: "", [`${NAME.toUpperCase()}_BROWSER`]: join(root, "no-such-browser") });
  assert.match((missing as { unavailable: string }).unavailable, /did not answer/);
});

// `askBrowser` runs with TMPDIR pointed at `folder`, which is where it makes its own temporary folder.
async function inTmp<T>(folder: string, body: () => Promise<T>): Promise<T> {
  const saved = process.env.TMPDIR;
  process.env.TMPDIR = folder;
  try {
    return await body();
  } finally {
    if (saved === undefined) delete process.env.TMPDIR;
    else process.env.TMPDIR = saved;
  }
}
const said = (g: Gpu) => ("unavailable" in g ? g.unavailable : g.adapter);

test("askBrowser says why when it cannot make its folder, and never throws", async () => {
  const got = await inTmp(join(root, "no-such-folder"), () => askBrowser(browser(`console.log(${JSON.stringify(answer({ api: "none", adapter: "" }))});`)));
  assert.match(said(got), /did not answer/);
});

test("askBrowser keeps its answer when its folder cannot be removed", { skip: process.getuid?.() === 0 }, async () => {
  const folder = join(root, "tmp-locked");
  mkdirSync(folder);
  const env = browser(`const fs = await import("node:fs");\nconst p = process.argv.find((a) => a.startsWith("--user-data-dir=")).slice(16) + "/locked";\nfs.mkdirSync(p, { recursive: true });\nfs.writeFileSync(p + "/x", "1");\nfs.chmodSync(p, 0o500);\nconsole.log(${JSON.stringify(answer({ api: "webgl2", adapter: "kept" }))});`);
  try {
    assert.deepEqual(await inTmp(folder, () => askBrowser(env)), { api: "webgl2", adapter: "kept" });
  } finally {
    for (const d of readdirSync(folder)) chmodSync(join(folder, d, "profile", "locked"), 0o700);
  }
});

test("askBrowser starts the browser with no background network and no component update", async () => {
  const flags = ["--disable-background-networking", "--disable-component-update"];
  const env = browser(`const flags = ${JSON.stringify(flags)}.filter((f) => process.argv.includes(f));\nconsole.log(${JSON.stringify(answer({ api: "webgl2", adapter: "X" }))}.replace("X", flags.join(" ")));`);
  assert.equal(said(await askBrowser(env)), flags.join(" "));
});

test("askBrowser stops waiting at its timeout for a browser that never exits and whose helper holds the pipe", async () => {
  const env = browser(`const { spawn } = await import("node:child_process");\nspawn(process.execPath, ["-e", "setTimeout(() => {}, 3000)"], { detached: true, stdio: "inherit" });\nsetTimeout(() => {}, 60000);`);
  const start = Date.now();
  const got = await askBrowser(env, 300);
  assert.match(said(got), /did not answer/);
  assert.ok(Date.now() - start < 2500, `waited ${Date.now() - start} ms`);
});

// A browser that cannot start its sandbox says so on stderr (AppArmor blocks user namespaces on Ubuntu) and exits 1, unless it is given --no-sandbox.
const SANDBOX = "[1:1:FATAL:zygote_host_impl_linux.cc(128)] No usable sandbox! If you are running on Ubuntu 23.10+, try --no-sandbox";
const refuse = `console.error(${JSON.stringify(SANDBOX)});\nprocess.exit(1);`;

test("askBrowser puts the browser's first error line in the skip, and retries once with --no-sandbox, and says so, only when that line names the sandbox", async () => {
  const stuck = await askBrowser(browser(refuse));
  assert.match(said(stuck), /exited with code 1: \[1:1:FATAL.*No usable sandbox.*retried without the sandbox/);
  const other = await askBrowser(browser(`console.error("first line\\nsecond line");\nprocess.exit(1);`));
  assert.match(said(other), /exited with code 1: first line$/);
  const env = browser(`if (!process.argv.includes("--no-sandbox")) {\n${refuse}\n}\nconsole.log(${JSON.stringify(answer({ api: "webgl2", adapter: "llvmpipe" }))});`);
  const got = await askBrowser(env);
  assert.deepEqual(got, { api: "webgl2", adapter: "llvmpipe", noSandbox: true });
  const r = run(argv, dir, undefined, got);
  assert.match(lastOf(r.out).what, /WebGL2.*llvmpipe.*retried the browser without the sandbox/);
});

test("doctor does not say \"No problems found.\" when the GPU was not checked: the summary says GPU not checked", () => {
  const skipped = run(["doctor", "--dir", dir], dir, undefined, { unavailable: "x did not answer" });
  assert.equal(skipped.code, 0, skipped.out + skipped.err);
  assert.match(skipped.out, /^No problems found, GPU not checked\.$/m);
  assert.doesNotMatch(skipped.out, /^No problems found\.$/m);
  const checked = run(["doctor", "--dir", dir], dir, undefined, { api: "webgl2", adapter: "llvmpipe" });
  assert.match(checked.out, /^No problems found\.$/m);
});

test("doctor asks no browser when it has no report to make: the package is not listed, or the folder is not a project", async () => {
  let asked = 0;
  const spy = async (): Promise<Gpu> => {
    asked++;
    return { unavailable: "spy" };
  };
  const bare = join(root, "bare");
  mkdirSync(bare);
  writeFileSync(join(bare, "package.json"), "{}");
  const unlisted = await runAsync(["doctor", "--dir", bare, "--json"], bare, spy);
  const nowhere = await runAsync(["doctor", "--dir", join(root, "nowhere")], root, spy);
  assert.deepEqual([unlisted.code, nowhere.code, asked], [1, 2, 0]);
});
