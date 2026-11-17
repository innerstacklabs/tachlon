// Runs the README's install prompt in one assistant and reports each check (gantt#1560, build row B6). One run, and it spends the assistant's
// quota: Claude is capped at 1 USD and Codex at a 600 s wall. Not in CI. Needs the package built (`bash scripts/build-web.sh --no-bundle` at the repo root, then `pnpm install --frozen-lockfile` and `pnpm build:lib` in packages/gantt) and a browser.
// Exit 0: every check passed. 1: a check failed. 2: the run could not happen (no assistant binary, no browser, no network, no build).
//   node scripts/agent-install-check.ts <claude|codex> [--json <results file>] [--record <dir>]
// `--record <dir>` keeps the assistant's stream, the page screenshot, the timings and a story file for the install video (scripts/lib/agent-install-record.ts).
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { chromePath } from "../packages/gantt/tests/support/knobs.ts";
import { launchRetrying } from "./lib/launch-retrying.ts";
import { distLibMissing, MOUNT_SECONDS, MOUNT_SELECTOR, PACKAGE } from "./lib/agent-install-check.ts";
import { agentPrompt } from "./lib/agent-prompt.ts";
import { record, streaming, timed } from "./lib/agent-install-record.ts";
import { ASSISTANTS, CouldNotRun, type Probes, runInstall, table } from "./lib/agent-install-run.ts";
import { serve } from "./lib/consumer-check.ts";
import { WEB_ROOT } from "./lib/web-root.ts";

const pkg = resolve(import.meta.dirname, "..", WEB_ROOT);

// `cmd` in the app folder, with the registry in its environment; the exit code and standard output.
const shell = (cmd: string, args: string[]) => (dir: string, env: NodeJS.ProcessEnv) => {
  try {
    return { code: 0, out: execFileSync(cmd, args, { cwd: dir, env, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"], timeout: 300_000, maxBuffer: 1 << 26 }) };
  } catch (e) {
    const { status, stdout } = e as { status: number | null; stdout?: string };
    return { code: status ?? 1, out: String(stdout ?? "") };
  }
};

// The built app (`dist/`) loaded in Chrome: the chart canvas's size, the seconds it took, the page errors; the page saved to `shot` when given.
async function mountProbe(dir: string, shot?: string) {
  const { chromium } = createRequire(join(pkg, "package.json"))("@playwright/test") as typeof import("@playwright/test");
  const browser = await launchRetrying(() => chromium.launch({ executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] }))
    .catch((e: Error) => { throw new CouldNotRun(`no browser: ${e.message.split("\n")[0]}`); });
  const { server, url } = await serve(join(dir, "dist"));
  try {
    const tab = await browser.newPage();
    let pageErrors = 0;
    tab.on("pageerror", () => pageErrors++);
    const start = Date.now();
    await tab.goto(url);
    const box = await tab.locator(MOUNT_SELECTOR).first().boundingBox({ timeout: MOUNT_SECONDS * 1000 }).catch(() => null);
    if (shot) await tab.screenshot({ path: shot });
    return { width: box?.width ?? 0, height: box?.height ?? 0, seconds: Math.round((Date.now() - start) / 1000), pageErrors };
  } finally {
    server.close();
    await browser.close();
  }
}

const usage = (): never => {
  console.error("usage: node scripts/agent-install-check.ts <claude|codex> [--json <results file>] [--record <dir>]");
  process.exit(2);
};
const { positionals, values: { json: file, record: recordDir } } = (() => {
  try { return parseArgs({ allowPositionals: true, options: { json: { type: "string" }, record: { type: "string" } } }); } catch { return usage(); }
})();
const name = positionals[0] ?? "";
const probes: Probes = {
  network: () => fetch("https://registry.npmjs.org/-/ping", { signal: AbortSignal.timeout(10_000) }).then(() => undefined, () => { throw new CouldNotRun("no network: npmjs did not answer"); }),
  build: shell("npm", ["run", "build"]),
  doctor: shell("npx", [PACKAGE, "doctor", "--json"]),
  mount: (dir) => mountProbe(dir, recordDir && join(recordDir, `page-${name}.png`)),
};

const assistant = ASSISTANTS[name];
if (!assistant || positionals.length > 1) usage();
const into = mkdtempSync(join(tmpdir(), "agent-install-tarball-"));
try {
  const missing = distLibMissing(pkg);
  if (missing) throw new CouldNotRun(missing);
  const [packed] = JSON.parse(execFileSync("npm", ["pack", "--json", "--ignore-scripts", "--pack-destination", into], { cwd: pkg, encoding: "utf8" }));
  const prompt = agentPrompt(readFileSync(join(pkg, "README.md"), "utf8"));
  const timings: Record<string, number> = {};
  if (recordDir) mkdirSync(recordDir, { recursive: true });
  const started = Date.now();
  const { work, results } = await runInstall(recordDir ? streaming(name, assistant) : assistant, prompt, { file: join(into, packed.filename), version: packed.version }, recordDir ? timed(probes, timings) : probes);
  timings.run = (Date.now() - started) / 1000;
  console.log(`${table(results)}\nfolder kept: ${work}`);
  writeFileSync(file ?? join(work, "results.json"), `${JSON.stringify({ assistant: name, work, results }, null, 2)}\n`);
  if (recordDir) record(recordDir, { assistant: name, work, results, timings });
  process.exitCode = results.every((r) => r.ok) ? 0 : 1;
} catch (e) {
  console.error(`agent-install-check: ${(e as Error).message}`);
  process.exitCode = 2;
} finally {
  rmSync(into, { recursive: true, force: true });
}
