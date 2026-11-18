// The install runner (gantt#1560, B6), driven by stub assistants (no real assistant, no network): one installs the package the way the prompt asks,
// one replaces the starter page, one pushes. The build, doctor and mount probes are stubbed; B4's tests judge what they report.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { PACKAGE } from "../../../scripts/lib/agent-install-check.ts";
import { argsFor, ASSISTANTS, type Assistant, CouldNotRun, type Probes, runInstall, table } from "../../../scripts/lib/agent-install-run.ts";

const root = mkdtempSync(join(tmpdir(), "agent-install-run-test-"));
after(() => rmSync(root, { recursive: true, force: true }));
// No git identity from the machine: the harness has to give the assistant its own.
process.env.GIT_CONFIG_GLOBAL = "/dev/null";
process.env.GIT_CONFIG_NOSYSTEM = "1";
const PORT = 4872; // not the registry's own 4871, which local-registry.test.ts holds
const tarball = { file: join(root, "pkg.tgz"), version: "9.9.9" };
mkdirSync(join(root, "package")); // the registry reads the tarball's package.json (gantt#1688)
writeFileSync(join(root, "package", "package.json"), "{}");
execFileSync("tar", ["-czf", tarball.file, "-C", root, "package"]);

// What an assistant does in the app folder (its working directory); `mode` picks the mistake. It keeps what it received next to the folder.
const stub = join(root, "stub.mjs");
writeFileSync(stub, `import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
const git = (...args) => execFileSync("git", args, { stdio: "pipe" });
writeFileSync("../received.txt", readFileSync(0, "utf8"));
const packument = await (await fetch(process.env.npm_config_registry + "/${PACKAGE}")).json();
const version = packument["dist-tags"].latest;
writeFileSync("../version.txt", version);
git("switch", "-c", "assistant");
const pkg = JSON.parse(readFileSync("package.json", "utf8"));
pkg.dependencies["${PACKAGE}"] = version;
writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\\n");
const page = readFileSync("src/App.tsx", "utf8");
writeFileSync("src/App.tsx", process.argv[2] === "replace" ? "export default function App() { return null; }\\n" : page + "\\nexport const chart = 1;\\n");
git("commit", "-q", "-am", "install");
if (process.argv[2] === "push") try { git("push", ".", "assistant:copy"); } catch {}
`);
const assistant = (mode: string): Assistant => ({ bin: process.execPath, args: [stub, mode] });
const probes = (seen: string[] = []): Probes => ({
  network: async () => {},
  build: (_dir, env) => (seen.push(String(env.npm_config_registry)), { code: 0 }),
  doctor: () => ({ code: 0, out: JSON.stringify({ findings: [{ code: "GPU_SKIPPED", status: "skipped" }] }) }),
  mount: async () => ({ width: 800, height: 400, seconds: 3, pageErrors: 0 }),
});
const failed = (results: { check: string; ok: boolean }[]) => results.filter((r) => !r.ok).map((r) => r.check);

test("a right install: the prompt arrives on stdin, the registry serves the version, every check passes, and the registry is closed after", async () => {
  const seen: string[] = [];
  const { work, results } = await runInstall(assistant("right"), "install it, please", tarball, probes(seen), { port: PORT });
  assert.equal(readFileSync(join(work, "received.txt"), "utf8"), "install it, please");
  assert.equal(readFileSync(join(work, "version.txt"), "utf8"), "9.9.9");
  assert.deepEqual(failed(results), []);
  assert.equal(results.length, 8);
  assert.deepEqual(seen, [`http://127.0.0.1:${PORT}`]);
  assert.match(table(results), /^PASS {2}branch/);
  await assert.rejects(fetch(`http://127.0.0.1:${PORT}/${PACKAGE}`));
});

test("an assistant that replaces the starter page fails that one check, and the table says so", async () => {
  const { results } = await runInstall(assistant("replace"), "x", tarball, probes(), { port: PORT });
  assert.deepEqual(failed(results), ["added, not replaced"]);
  assert.match(table(results), /FAIL {2}added, not replaced\s+.*lost 1\d\d lines/);
});

test("an assistant that pushes is caught by the pre-push hook", async () => {
  const { results } = await runInstall(assistant("push"), "x", tarball, probes(), { port: PORT });
  assert.deepEqual(failed(results), ["no push"]);
});

test("could not run: a missing assistant binary and a failed network probe stop the run before any check", async () => {
  await assert.rejects(runInstall({ bin: "no-such-assistant-binary", args: [] }, "x", tarball, probes(), { port: PORT }), CouldNotRun);
  const noNetwork: Probes = { ...probes(), network: async () => { throw new CouldNotRun("no network"); } };
  await assert.rejects(runInstall(assistant("right"), "x", tarball, noNetwork, { port: PORT }), /no network/);
});

test("Codex gets the app's git folder before the prompt argument, and Claude does not", () => {
  assert.deepEqual(argsFor(ASSISTANTS.codex, "/app").slice(-3), ["--add-dir", "/app/.git", "-"]);
  assert.equal(argsFor(ASSISTANTS.claude, "/app").includes("--add-dir"), false);
});
