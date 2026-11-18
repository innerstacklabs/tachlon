// The runner for the install check (gantt#1560, build row B6): the README's prompt, run by one assistant (`claude -p` or `codex exec`) in a temp copy
// of the test app that has a git history on `main` and a `pre-push` hook, with the unreleased package served by a local registry; then B4's checks
// judge the folder. A check that fails is a result; a run that cannot happen (no assistant binary, no browser, no network, no port) throws CouldNotRun.
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdtempSync, openSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { branch, build, dependency, doctor, mount, noPush, PUSH_MARKER, replacedPage, type Result } from "./agent-install-check.ts";
import { startRegistry } from "./local-registry.ts";

export class CouldNotRun extends Error {}
export type Assistant = { bin: string; args: string[] }; // the prompt arrives on its stdin
export type Probes = {
  network: () => Promise<void>;
  build: (dir: string, env: NodeJS.ProcessEnv) => { code: number } | Promise<{ code: number }>;
  doctor: (dir: string, env: NodeJS.ProcessEnv) => { code: number; out: string } | Promise<{ code: number; out: string }>;
  mount: (dir: string) => Promise<Parameters<typeof mount>[0]>;
};

export const WALL_MS = 600_000; // one run, ten minutes at most
const TOOLS = ["Bash(npm *)", "Bash(npx *)", "Bash(git *)", "Bash(node *)", "Read", "Edit", "Write", "Glob", "Grep"];
export const ASSISTANTS: Record<string, Assistant> = {
  claude: { bin: "claude", args: ["-p", "--model", "sonnet", "--setting-sources", "project", "--strict-mcp-config", "--disable-slash-commands", "--permission-mode", "acceptEdits", "--allowedTools", TOOLS.join(","), "--max-budget-usd", "1"] },
  codex: { bin: "codex", args: ["exec", "-s", "workspace-write", "-c", "sandbox_workspace_write.network_access=true", "-"] },
};
/** Codex gets the app's git folder writable, so the check measures whether it makes the branch and the commit, not whether its sandbox lets it. */
export const argsFor = (assistant: Assistant, dir: string): string[] => assistant.bin === "codex" ? [...assistant.args.slice(0, -1), "--add-dir", join(dir, ".git"), "-"] : assistant.args;
const FIXTURE = resolve(import.meta.dirname, "../fixtures/agent-install/app");
const git = (dir: string, ...args: string[]) => execFileSync("git", ["-C", dir, ...args], { stdio: "pipe" });

/** Runs `assistant` on `prompt` in a fresh copy of the test app; `work` holds `app/`, the assistant's log and any file a caller wants kept. */
export async function runInstall(assistant: Assistant, prompt: string, tarball: { file: string; version: string }, probes: Probes, options: { port?: number; scaffold?: (dir: string) => void; page?: string; then?: string } = {}): Promise<{ work: string; results: Result[]; later: Result[][] }> {
  const work = mkdtempSync(join(tmpdir(), "agent-install-run-"));
  const dir = join(work, "app");
  (options.scaffold ?? ((to) => cpSync(FIXTURE, to, { recursive: true })))(dir);
  git(dir, "init", "-q", "-b", "main");
  git(dir, "config", "user.name", "install-check");
  git(dir, "config", "user.email", "install-check@example.invalid");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "starter");
  writeFileSync(join(dir, ".git/hooks/pre-push"), `#!/bin/sh\ntouch '${join(dir, PUSH_MARKER)}'\nexit 1\n`, { mode: 0o755 });
  await probes.network();
  const port = options.port ?? 4871;
  const registry = await startRegistry(tarball.file, tarball.version, port).catch((e: Error) => { throw new CouldNotRun(`the local registry did not start: ${e.message}`); });
  let wall: NodeJS.Timeout | undefined;
  const env = { ...process.env, npm_config_registry: `http://127.0.0.1:${port}`, TELEGRAM_BOT_TOKEN: undefined, TELEGRAM_STATE_DIR: undefined }; // a child assistant must not reach its parent pane's chat
  try {
    const ask = async (text: string, n: number) => { // each ask is a fresh session in the same folder; the log of the first is assistant.log
      const was = String(git(dir, "rev-parse", "HEAD"));
      const log = openSync(join(work, n ? `assistant-${n + 1}.log` : "assistant.log"), "w");
      const child = spawn(assistant.bin, argsFor(assistant, dir), { cwd: dir, env, stdio: ["pipe", log, log] }); // async: this process serves the registry
      wall = setTimeout(() => child.kill(), WALL_MS);
      child.stdin?.end(text);
      await new Promise((done, fail) => {
        child.once("error", (e: NodeJS.ErrnoException) => fail(e.code === "ENOENT" ? new CouldNotRun(`${assistant.bin} is not installed`) : e));
        child.once("close", done);
      });
      clearTimeout(wall);
      const moved = String(git(dir, "rev-parse", "HEAD")) !== was;
      return [branch(dir), replacedPage(dir, options.page), dependency(dir), build(await probes.build(dir, env)), doctor(await probes.doctor(dir, env)), mount(await probes.mount(dir)), noPush(dir), { check: "new commit", ok: moved, why: moved ? "the head moved" : "the session made no commit" }];
    };
    const results = await ask(prompt, 0);
    const later = options.then ? [await ask(options.then, 1)] : [];
    return { work, results, later };
  } finally {
    clearTimeout(wall);
    registry.close();
  }
}

/** One line for each check. */
export const table = (results: Result[]): string => results.map((r) => `${r.ok ? "PASS" : "FAIL"}  ${r.check.padEnd(20)}${r.why}`).join("\n");
