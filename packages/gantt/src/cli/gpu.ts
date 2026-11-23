// What `doctor` asks a real browser about the GPU: it opens a page of its own, headless, in a Chrome, Edge or Chromium found on this machine, and reads the answer the page writes.
// Only this file starts a browser; `doctor.ts` turns what it returns into a finding.
import { spawn } from "node:child_process";
import { accessSync, constants, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { delimiter, extname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { PRODUCT_NAME } from "../product.ts";

/** What the browser said: the graphics path the chart takes (WebGPU first, then WebGL2, as `mount` tries them) and the adapter's name; or why no browser could say. `draw` is what a page showed in the same browser, when it was asked. */
export type Gpu = ({ api: "webgpu" | "webgl2" | "none"; adapter: string; /** True when the browser only started after a retry with `--no-sandbox`. */ noSandbox?: true } | { unavailable: string }) & { draw?: Draw };
/** What a page had drawn when the browser dumped it: the task rows, and the timeline canvas's size in pixels; or why no browser could say. `page` is the address it loaded, or says it was the built-in sample. */
export type Draw = { page: string; rows: number; width: number; height: number } | { unavailable: string };

/** The environment variable that names the browser to ask, for a machine where it is not on the path. */
export const BROWSER_VARIABLE = `${PRODUCT_NAME.toUpperCase()}_BROWSER`;
const NAMES = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge"];
const APPS = ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"];
const PLAYWRIGHT_PROGRAMS = ["chrome-linux64/chrome", "chrome-linux/chrome", "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing", "chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing", "chrome-win64/chrome.exe", "chrome-win/chrome.exe"];
const TIMEOUT_MS = 30_000;
/** The page: it asks as `mount` does, and writes the answer into its title, which `--dump-dom` prints. A file address is a secure context, where `navigator.gpu` exists. */
const PAGE = `<!doctype html><title></title><script>
(async () => {
  const adapter = await navigator.gpu?.requestAdapter().catch(() => null);
  if (adapter) return { api: "webgpu", adapter: [adapter.info.vendor, adapter.info.architecture, adapter.info.description].filter(Boolean).join(" ") || "unnamed adapter" };
  const gl = document.createElement("canvas").getContext("webgl2");
  if (!gl) return { api: "none", adapter: "" };
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  return { api: "webgl2", adapter: String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER)) };
})().then((answer) => (document.title = "gpu=" + encodeURIComponent(JSON.stringify(answer))));
</script>`;

/** The built-in sample page: the package's own script, stylesheet and engine, and two tasks in a box with a size. */
const SAMPLE = `<!doctype html><link rel="stylesheet" href="/gantt.css"><div class="gantt" style="width:800px;height:300px"></div><script type="module">import { mount } from "/${PRODUCT_NAME.toLowerCase()}.js"; mount(document.querySelector(".gantt"), { tasks: [{ id: 1, name: "Design", start: "2026-11-02", duration: 3 }, { id: 2, name: "Build", start: "2026-11-05", duration: 5 }], links: [{ from: 1, to: 2 }] });</script>`;

const why = (e: unknown): string => (e instanceof Error ? e.message : String(e)).replace(/\s+/g, " ").slice(0, 200);

const runnable = (file: string) => {
  try {
    accessSync(file, constants.X_OK);
    return true;
  } catch {
    return false;
  }
};

/** A Chromium Playwright installed: the newest `chromium-<build>` folder of its cache (`PLAYWRIGHT_BROWSERS_PATH`, else its usual place on Linux, a Mac or Windows) that holds the program. */
function playwrightChromium(env: NodeJS.ProcessEnv): string | undefined {
  const home = env.HOME ?? env.USERPROFILE ?? "";
  const caches = env.PLAYWRIGHT_BROWSERS_PATH ? [env.PLAYWRIGHT_BROWSERS_PATH] : [join(home, ".cache/ms-playwright"), join(home, "Library/Caches/ms-playwright"), join(env.LOCALAPPDATA ?? home, "ms-playwright")];
  const builds = caches.flatMap((cache) => {
    try {
      return readdirSync(cache).filter((d) => /^chromium-\d+$/.test(d)).map((d) => join(cache, d));
    } catch {
      return [];
    }
  });
  const number = (build: string) => Number(build.slice(build.lastIndexOf("-") + 1));
  return builds.sort((a, b) => number(b) - number(a)).flatMap((b) => PLAYWRIGHT_PROGRAMS.map((p) => join(b, p))).find(runnable);
}

/** The browser to ask: the one the environment variable names, else the first of the usual names on the path, else the usual place of an app on a Mac, else a Chromium Playwright installed. */
function findBrowser(env: NodeJS.ProcessEnv): string | undefined {
  if (env[BROWSER_VARIABLE]) return env[BROWSER_VARIABLE];
  const onPath = (env.PATH ?? "").split(delimiter).filter(Boolean).flatMap((d) => NAMES.map((n) => join(d, n)));
  return [...onPath, ...APPS].find(runnable) ?? playwrightChromium(env);
}

/** The first line `stderr` holds, the one that says why a browser failed to start. */
const firstLine = (stderr: string): string => stderr.split("\n").map((l) => l.trim()).find(Boolean) ?? "";

/** What `browser` prints with `args`, or an error once `ms` pass; one that exits with a code says its first line of stderr, kept as `stderr` on the error. It runs as a process group that a SIGKILL ends whole, and the wait ends on the timer even when a helper of the browser still holds the output pipe open. */
function output(browser: string, args: string[], ms: number): Promise<string> {
  let timer: NodeJS.Timeout | undefined;
  return new Promise<string>((done, fail) => {
    const child = spawn(browser, args, { detached: true, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => (stdout += chunk));
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => (stderr = (stderr + chunk).slice(0, 4000)));
    child.on("error", fail).on("close", (code) => {
      const line = firstLine(stderr);
      return code === 0 ? done(stdout) : fail(Object.assign(new Error(`exited with code ${code}${line ? `: ${line}` : ""}`), { stderr: line }));
    });
    timer = setTimeout(() => {
      try {
        process.kill(-(child.pid as number), "SIGKILL");
      } catch {
        /* the group is gone */
      }
      fail(new Error(`no answer in ${ms / 1000} s`));
    }, ms);
  }).finally(() => clearTimeout(timer));
}

/** Opens the address `open` makes (in a folder of its own, which is removed after) in the browser `env` leads to and returns what `read` makes of the page the browser dumped; `read` has nothing to say when it returns `undefined`, and `silent` says what that means. No browser, one that fails or does not answer in `timeoutMs`, or one with nothing to say is `unavailable`, with the reason. It never throws. */
async function inBrowser<T>(env: NodeJS.ProcessEnv, timeoutMs: number, open: (folder: string) => Promise<string>, read: (dump: string, noSandbox: boolean) => T | undefined, silent: string): Promise<T | { unavailable: string }> {
  const browser = findBrowser(env);
  if (!browser) return { unavailable: `no Chrome, Edge or Chromium was found on this machine or in Playwright's cache (set ${BROWSER_VARIABLE} to the path of one)` };
  let dir: string | undefined;
  try {
    const folder = (dir = mkdtempSync(join(tmpdir(), "doctor-gpu-")));
    const address = await open(folder);
    const flags = (profile: string, more: string[] = []) => ["--headless=new", "--no-first-run", "--disable-background-networking", "--disable-component-update", `--user-data-dir=${join(folder, profile)}`, ...more, "--virtual-time-budget=10000", "--dump-dom", address];
    let stdout: string;
    let noSandbox = false;
    try {
      stdout = await output(browser, flags("profile"), timeoutMs);
    } catch (e) {
      // A browser that cannot start its sandbox (AppArmor blocks user namespaces on Ubuntu) is asked once more without it; any other failure is the answer.
      if (!/sandbox/i.test((e as { stderr?: string }).stderr ?? "")) throw e;
      try {
        stdout = await output(browser, flags("profile-no-sandbox", ["--no-sandbox"]), timeoutMs);
        noSandbox = true;
      } catch (again) {
        return { unavailable: `${browser} did not answer: ${why(e)}; retried without the sandbox, which also failed: ${why(again)}` };
      }
    }
    return read(stdout, noSandbox) ?? { unavailable: `${browser} ran the page but ${silent}` };
  } catch (e) {
    return { unavailable: `${browser} did not answer: ${why(e)}` };
  } finally {
    // A folder left behind is not worth losing the answer for.
    if (dir) await rm(dir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 }).catch(() => undefined);
  }
}

/** Opens the GPU page in the browser `env` leads to and returns what it said; no browser, one that fails or does not answer in `timeoutMs`, or one that prints no report is `unavailable`, with the reason. It never throws. */
export function askBrowser(env: NodeJS.ProcessEnv = process.env, timeoutMs = TIMEOUT_MS): Promise<Gpu> {
  const open = async (folder: string) => {
    const page = join(folder, "gpu.html");
    writeFileSync(page, PAGE);
    return pathToFileURL(page).href;
  };
  return inBrowser(env, timeoutMs, open, (dump, noSandbox): Gpu | undefined => {
    const report = /<title>gpu=([^<]*)<\/title>/.exec(dump)?.[1];
    const got = report === undefined ? undefined : (JSON.parse(decodeURIComponent(report)) as Partial<Extract<Gpu, { api: string }>>);
    if (got?.api !== "webgpu" && got?.api !== "webgl2" && got?.api !== "none") return undefined;
    return { api: got.api, adapter: String(got.adapter ?? ""), ...(noSandbox ? { noSandbox: true as const } : {}) };
  }, "sent no GPU report");
}

/** What `doctor` asks a browser: the GPU, and, when one answered, what `page` (the built-in sample when none) drew. */
export async function askBoth(page: string | undefined): Promise<Gpu> {
  const gpu = await askBrowser();
  return { ...gpu, draw: "api" in gpu ? await askDraw(page) : { unavailable: gpu.unavailable } };
}

/** The built-in sample, a page that mounts the chart of the package this command came from: a server of its own on this machine serves it and the package's `dist-lib`, because a browser runs a module and fetches the engine from a file address only with its security off. The server never keeps the process alive. */
async function sample(): Promise<string> {
  const root = fileURLToPath(new URL("../../dist-lib/", import.meta.url));
  const types: Record<string, string> = { ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm" };
  const server = createServer((request, response) => {
    const path = new URL(request.url ?? "/", "http://x").pathname;
    try {
      const body = path === "/" ? SAMPLE : readFileSync(join(root, path));
      response.writeHead(200, { "content-type": types[extname(path)] ?? "text/html" }).end(body);
    } catch {
      response.writeHead(404).end();
    }
  }).unref();
  await new Promise<void>((listening) => server.listen(0, "127.0.0.1", listening));
  return `http://127.0.0.1:${(server.address() as { port: number }).port}/`;
}

/** Loads `page` (the built-in sample when none) in the browser `env` leads to, and counts what the dumped page holds: the task rows, and the timeline canvas's width and height. A page that drew nothing reads as 0, not as `unavailable`. It never throws. */
export function askDraw(page: string | undefined, env: NodeJS.ProcessEnv = process.env, timeoutMs = TIMEOUT_MS): Promise<Draw> {
  const open = async () => page ?? sample();
  return inBrowser(env, timeoutMs, open, (dump): Draw => {
    const canvas = /<canvas\b[^>]*\bdata-part="timeline"[^>]*>/.exec(dump)?.[0] ?? "";
    const size = (name: string) => Number(new RegExp(`\\s${name}="(\\d+)"`).exec(canvas)?.[1] ?? 0);
    return { page: page ?? "the built-in sample", rows: (dump.match(/<div class="row\b[^>]*\sdata-task=/g) ?? []).length, width: size("width"), height: size("height") };
  }, "dumped nothing");
}
