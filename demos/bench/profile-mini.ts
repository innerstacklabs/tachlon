// The steps of `profile.ts` that touch the Mac mini (`ssh <host>`), in one module so a test can swap it. A session is a
// fresh Chromium on the mini, found through its own DevToolsActivePort file (port 0, so no port is assumed and no other
// browser is ever attached to) with that port forwarded to the hub, where Playwright drives it. The bundle is copied
// to the mini and served there on 127.0.0.1 (`python3 -m http.server`, as MEASURING.md does), so no phase includes a
// hub-to-mini transfer. Every process and directory the session makes is removed by `close`, by pid and path, never by pattern.

import { type ChildProcess, execFile, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { createServer } from "node:net";
import { promisify } from "node:util";
import { chromium } from "./drive.ts";
import { parseFootprint, parseSmapsRollup } from "./memory.ts";
import { isOwnDir, ownPids } from "./profile-format.ts";

const run = promisify(execFile);

/** What `profile.ts` needs of the mini. */
export interface Session {
  /** The browser Playwright drives (on the mini). */
  browser: any;
  /** The address of the bundle's folder, as the mini's browser reaches it. */
  baseUrl: string;
  /** The physical footprint, kB, of each pid on the mini (`vmmap --summary`). */
  footprintKb(pids: number[]): Promise<Map<number, number>>;
  close(): Promise<void>;
}
/** `register` is given the session's closer before the first remote process starts, so a signal during the open can run it. */
export type OpenSession = (o: { host: string; chrome: string; bundle: string; register?: (close: () => Promise<void>) => void }) => Promise<Session>;

const q = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
/** `mktemp -d` of a marked name (`gantt-profile.<id>`) in the host's temp directory: what marks everything this command makes there. */
const MKTEMP = 'mktemp -d "${TMPDIR:-/tmp}/gantt-profile.XXXXXX"';
const SSH = ["-o", "ConnectTimeout=5"];
const LINUX_ENV = "env XDG_RUNTIME_DIR=/run/user/$(id -u) WAYLAND_DISPLAY=wayland-1 __NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia __VK_LAYER_NV_optimus=NVIDIA_only ";
const LINUX_FLAGS = "--ignore-gpu-blocklist --ozone-platform=wayland --enable-unsafe-webgpu --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan --use-angle=vulkan ";
const freePort = () => new Promise<number>((resolve) => { const s = createServer().listen(0, "127.0.0.1", () => { const p = (s.address() as { port: number }).port; s.close(() => resolve(p)); }); });

const stopScript = (pid: string) => `kill ${pid}; for i in $(seq 1 25); do kill -0 ${pid} 2>/dev/null || exit 0; sleep 0.2; done; kill -9 ${pid}`;

/** Remove what an earlier run left on the host (it was stopped hard, so no handler ran): the processes whose command line holds a marked path, then the marked directories, and nothing without the marker. */
export async function sweepLeftovers(host: string): Promise<void> {
  const ssh = async (cmd: string) => (await run("ssh", [...SSH, host, cmd], { encoding: "utf8" })).stdout;
  for (const pid of ownPids(await ssh("ps -axo pid=,command=").catch(() => ""))) await ssh(stopScript(pid)).catch(() => undefined);
  for (const dir of (await ssh('ls -d "${TMPDIR:-/tmp}"/gantt-profile.* 2>/dev/null').catch(() => "")).split("\n")) if (isOwnDir(dir.trim())) await ssh(`rm -rf ${q(dir.trim())}`).catch(() => undefined);
}

/** `script` on the host after `echo $$` (the pid `exec` keeps); the pid and a reader of its next stdout lines. */
async function start(host: string, script: string): Promise<{ proc: ChildProcess; pid: string; line: () => Promise<string> }> {
  const proc = spawn("ssh", [...SSH, host, `echo $$; ${script}`], { stdio: ["ignore", "pipe", "ignore"] });
  const lines = createInterface({ input: proc.stdout! })[Symbol.asyncIterator]();
  const line = async () => String((await lines.next()).value ?? "");
  return { proc, pid: await line(), line };
}

export const openSession: OpenSession = async ({ host, chrome, bundle, register }) => {
  const ssh = async (cmd: string) => (await run("ssh", [...SSH, host, cmd], { encoding: "utf8" })).stdout.trim();
  /** Stop a process this session started: TERM, wait up to 5 s for it to go, then KILL, so nothing it owns outlives the directory removal. */
  const stop = async (pid: string) => void (/^\d+$/.test(pid) && (await ssh(stopScript(pid)).catch(() => undefined)));
  const cleanup: (() => Promise<unknown>)[] = [];
  // One close for everyone: the handler's, `profile.ts`'s and the failure path's wait on the same run.
  let closing: Promise<void> | undefined;
  const close = () => (closing ??= runAll(cleanup));
  register?.(close);
  try {
    await sweepLeftovers(host);
    const site = await ssh(MKTEMP);
    cleanup.push(() => (isOwnDir(site) ? ssh(`rm -rf ${q(site)}`) : Promise.resolve()));
    await new Promise<void>((resolve, reject) => {
      const tar = spawn("sh", ["-c", `tar -C ${q(bundle)} -cf - . | ssh -o ConnectTimeout=5 ${q(host)} ${q(`tar -C ${q(site)} -xf -`)}`], { stdio: "ignore" });
      cleanup.push(async () => void tar.kill());
      tar.on("exit", (c) => (c === 0 ? resolve() : reject(new Error(`copying the bundle to ${host} failed (${c})`))));
    });
    const web = await start(host, `exec python3 -u -m http.server 0 --bind 127.0.0.1 --directory ${q(site)}`);
    cleanup.push(() => stop(web.pid));
    const port = (await web.line()).match(/port (\d+)/)?.[1];
    if (!port) throw new Error(`the page server on ${host} did not say its port`);

    const profile = await ssh(MKTEMP);
    cleanup.push(() => (isOwnDir(profile) ? ssh(`rm -rf ${q(profile)}`) : Promise.resolve()));
    // A Linux host draws on its GPU only in a headed window of the logged-in session, with the flags and offload variables MEASURING.md's GPU rows use; headless over ssh gives SwiftShader.
    const linux = (await ssh("uname")) === "Linux";
    const chr = await start(host, `exec ${linux ? LINUX_ENV : ""}${q(chrome)} --remote-debugging-port=0 --user-data-dir=${q(profile)} --no-first-run --no-default-browser-check ${linux ? LINUX_FLAGS : ""}about:blank`);
    cleanup.push(() => stop(chr.pid));
    let cdp = "";
    for (let i = 0; i < 60 && !cdp; i++) {
      await new Promise((r) => setTimeout(r, 500));
      cdp = (await ssh(`head -n 1 ${q(`${profile}/DevToolsActivePort`)} 2>/dev/null`).catch(() => "")).trim();
    }
    if (!/^\d+$/.test(cdp)) throw new Error(`the Chromium started on ${host} did not report a debugging port`);
    const local = await freePort();
    const tunnel = spawn("ssh", ["-N", ...SSH, "-o", "ExitOnForwardFailure=yes", "-L", `${local}:127.0.0.1:${cdp}`, host], { stdio: "ignore" });
    cleanup.push(async () => void tunnel.kill());
    let browser: any;
    for (let i = 0; i < 40 && !browser; i++) {
      await new Promise((r) => setTimeout(r, 500));
      browser = await chromium().connectOverCDP(`http://127.0.0.1:${local}`).catch(() => undefined);
    }
    if (!browser) throw new Error(`no Chromium answered through the tunnel to ${host}`);
    cleanup.push(() => browser.close().catch(() => undefined));
    return {
      browser,
      baseUrl: `http://127.0.0.1:${port}/`,
      async footprintKb(pids) {
        const out = new Map<number, number>();
        for (const pid of pids) {
          const kb = linux ? parseSmapsRollup(await ssh(`cat /proc/${pid}/smaps_rollup`)) : parseFootprint(await ssh(`LC_ALL=C vmmap --summary ${pid}`));
          if (kb !== null) out.set(pid, kb);
        }
        return out;
      },
      close,
    };
  } catch (e) {
    await close();
    throw e;
  }
};

/** Run the cleanups last-made first, each one even when another throws. */
async function runAll(steps: (() => Promise<unknown>)[]): Promise<void> {
  for (const s of steps.splice(0).reverse()) await s().catch(() => undefined);
}
