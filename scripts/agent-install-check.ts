// Runs the README's install prompt in one assistant and reports each check (gantt#1560, build row B6). One run, and it spends the assistant's
// quota: Claude is capped at 1 USD and Codex at a 600 s wall. Not in CI. Needs the package built (`bash scripts/build-web.sh --no-bundle` at the repo root, then `pnpm install --frozen-lockfile` and `pnpm build:lib` in packages/gantt) and a browser.
// Exit 0: every check passed. 1: a check failed. 2: the run could not happen (no assistant binary, no browser, no network, no build).
//   node scripts/agent-install-check.ts <claude|codex> [--app <app>] [--fit] [--version <v>] [--json <results file>] [--record <dir>]
// `--app` picks the starter app (scripts/lib/agent-install-apps.ts; default vite-react); `--fit` then gives a fresh session the fit task; `--version <v>` tests the published
// package of that version, fetched with `npm pack`, and needs no build. The last line printed is a row for docs/agent-install-matrix.md (the matrix's command is in that file).
// `--record <dir>` keeps the assistant's stream, the page screenshot, the timings and a story file for the install video (scripts/lib/agent-install-record.ts).
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { chromePath } from "../packages/gantt/tests/support/knobs.ts";
import { launchRetrying } from "./lib/launch-retrying.ts";
import { distLibMissing, MOUNT_SECONDS, MOUNT_SELECTOR, PACKAGE, type Result } from "./lib/agent-install-check.ts";
import { agentPrompt } from "./lib/agent-prompt.ts";
import { APPS, FIT_PROMPT, open } from "./lib/agent-install-apps.ts";
import { record, streaming, timed } from "./lib/agent-install-record.ts";
import { ASSISTANTS, CouldNotRun, type Probes, runInstall, table } from "./lib/agent-install-run.ts";
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
async function mountProbe(dir: string, how: string, shot?: string) {
  const { chromium } = createRequire(join(pkg, "package.json"))("@playwright/test") as typeof import("@playwright/test");
  const browser = await launchRetrying(() => chromium.launch({ executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] }))
    .catch((e: Error) => { throw new CouldNotRun(`no browser: ${e.message.split("\n")[0]}`); });
  const { url, close } = await open(dir, how);
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
    close();
    await browser.close();
  }
}

// A row of docs/agent-install-matrix.md: each session's checks, then its turns and minutes from its own result line (only a streamed run, with `--record`, has one).
const cell = (label: string, phase: Result[], log: string): string => {
  const last = readFileSync(log, "utf8").split("\n").filter((l) => l.startsWith("{")).map((l) => JSON.parse(l)).findLast((x) => x.type === "result");
  const spent = last ? `${last.num_turns} turns, ${(last.duration_ms / 60_000).toFixed(1)} min${last.subtype === "success" ? "" : `, stopped: ${last.subtype}`}` : "no turn count";
  return `${label}: ${phase.every((r) => r.ok) ? "pass" : `FAIL (${phase.filter((r) => !r.ok).map((r) => r.check).join(", ")})`}, ${spent}`;
};
const usage = (): never => {
  console.error(`usage: node scripts/agent-install-check.ts <claude|codex> [--app <${Object.keys(APPS).join("|")}>] [--fit] [--version <published version>] [--json <results file>] [--record <dir>]`);
  process.exit(2);
};
const { positionals, values: { json: file, record: recordDir, app: appId = "vite-react", version, fit } } = (() => {
  try { return parseArgs({ allowPositionals: true, options: { json: { type: "string" }, record: { type: "string" }, app: { type: "string" }, version: { type: "string" }, fit: { type: "boolean" } } }); } catch { return usage(); }
})();
const name = positionals[0] ?? "";
const app = APPS[appId];
let shots = 0;
const probes: Probes = {
  network: () => fetch("https://registry.npmjs.org/-/ping", { signal: AbortSignal.timeout(10_000) }).then(() => undefined, () => { throw new CouldNotRun("no network: npmjs did not answer"); }),
  build: shell("npm", ["run", "build"]),
  doctor: shell("npx", [PACKAGE, "doctor", "--json"]),
  mount: (dir) => mountProbe(dir, app!.serve, recordDir && join(recordDir, `page-${name}${shots++ ? "-fit" : ""}.png`)),
};

const assistant = ASSISTANTS[name];
if (!assistant || !app || positionals.length > 1) usage();
const into = mkdtempSync(join(tmpdir(), "agent-install-tarball-"));
try {
  const missing = version ? undefined : distLibMissing(pkg);
  if (missing) throw new CouldNotRun(missing);
  const [packed] = JSON.parse(execFileSync("npm", ["pack", ...(version ? [`${PACKAGE}@${version}`] : []), "--json", "--ignore-scripts", "--pack-destination", into], { cwd: version ? into : pkg, encoding: "utf8" }));
  const prompt = agentPrompt(readFileSync(join(pkg, "README.md"), "utf8"));
  const timings: Record<string, number> = {};
  if (recordDir) mkdirSync(recordDir, { recursive: true });
  const started = Date.now();
  const { work, results, later } = await runInstall(recordDir ? streaming(name, assistant) : assistant, prompt, { file: join(into, packed.filename), version: packed.version }, recordDir ? timed(probes, timings) : probes, { ...app, then: fit ? FIT_PROMPT : undefined });
  timings.run = (Date.now() - started) / 1000;
  console.log(`${[results, ...later].map(table).join("\n--- then the fit task ---\n")}\nfolder kept: ${work}`);
  console.log(`| ${appId} | ${name} | ${[cell("install", results, join(work, "assistant.log")), ...later.map((l) => cell("fit", l, join(work, "assistant-2.log")))].join("<br>")} |`);
  writeFileSync(file ?? join(work, "results.json"), `${JSON.stringify({ assistant: name, app: appId, version: packed.version, work, results, later }, null, 2)}\n`);
  if (recordDir) record(recordDir, { assistant: name, work, results, timings });
  process.exitCode = [results, ...later].every((phase) => phase.every((r) => r.ok)) ? 0 : 1;
} catch (e) {
  console.error(`agent-install-check: ${(e as Error).message}`);
  process.exitCode = 2;
} finally {
  rmSync(into, { recursive: true, force: true });
}
