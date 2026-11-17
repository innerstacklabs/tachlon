import { existsSync } from "node:fs";
import { mkdir, readdir, readFile, symlink } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import * as script from "../steps.mjs";
import { createContext } from "../ctx.mjs";
import { ffmpegPath } from "./media.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");

// The first import finds a standalone playwright-core next to the script (the MacBook Pro copy under ~/gantt-probe);
// in the repo, packages/gantt/'s @playwright/test is the only Playwright package.
export async function loadChromium() {
  try {
    return (await import("playwright-core")).chromium;
  } catch {
    return createRequire(join(root, "packages", "gantt", "package.json"))("@playwright/test").chromium;
  }
}

// packages/gantt/ installs @playwright/test, which pulls in playwright, which pulls in playwright-core; pnpm exposes none of
// the inner two to packages/gantt/, so each hop resolves from the package before it.
function resolveFromWeb() {
  const webRequire = createRequire(join(root, "packages", "gantt", "package.json"));
  const playwrightRequire = createRequire(webRequire.resolve("@playwright/test"));
  return createRequire(playwrightRequire.resolve("playwright")).resolve("playwright-core/package.json");
}

// recordVideo runs the ffmpeg that playwright-core's browsers.json names (a revision the cache may lack or hold in an
// older build without VP9). Point that path at the system ffmpeg through a private browsers directory in the temp
// directory, so nothing is installed; the browser itself is launched by executablePath.
export async function useSystemFfmpegForVideo(temp) {
  if (process.env.PLAYWRIGHT_BROWSERS_PATH) return;
  let pkg;
  for (const find of [() => createRequire(import.meta.url).resolve("playwright-core/package.json"), resolveFromWeb]) {
    try {
      pkg = find();
      break;
    } catch {
      // try the next place
    }
  }
  if (!pkg) return;
  const wanted = JSON.parse(await readFile(join(dirname(pkg), "browsers.json"), "utf8")).browsers.find((b) => b.name === "ffmpeg")?.revision;
  if (!wanted) return;
  const target = join(temp, "pw-browsers", `ffmpeg-${wanted}`);
  await mkdir(target, { recursive: true });
  await symlink(ffmpegPath(), join(target, platform() === "darwin" ? "ffmpeg-mac" : "ffmpeg-linux"));
  process.env.PLAYWRIGHT_BROWSERS_PATH = join(temp, "pw-browsers");
  console.log(`recordVideo: using ${ffmpegPath()} as Playwright's ffmpeg-${wanted}`);
}

export async function chromePath() {
  if (process.env.CHROME) return process.env.CHROME;
  const system = platform();
  const base = system === "darwin"
    ? join(homedir(), "Library", "Caches", "ms-playwright")
    : system === "linux"
      ? join(homedir(), ".cache", "ms-playwright")
      : null;
  if (!base) throw new Error(`no headless Chromium lookup for ${system}; set CHROME`);
  let names;
  try {
    names = await readdir(base);
  } catch {
    throw new Error(`no Playwright cache at ${base}; set CHROME`);
  }
  const newest = names
    .map((name) => ({ name, version: Number(/^chromium_headless_shell-(\d+)$/.exec(name)?.[1] ?? -1) }))
    .filter(({ version }) => version >= 0)
    .sort((a, b) => b.version - a.version)[0]?.name;
  if (!newest) throw new Error(`no chromium_headless_shell-<N> under ${base}; set CHROME`);
  if (system === "darwin" && process.arch !== "arm64") throw new Error(`no default headless shell for darwin ${process.arch}; set CHROME`);
  const binary = system === "darwin"
    ? join(base, newest, "chrome-headless-shell-mac-arm64", "chrome-headless-shell")
    : join(base, newest, "chrome-headless-shell-linux64", "chrome-headless-shell");
  if (!existsSync(binary)) throw new Error(`headless Chromium not found at ${binary}; set CHROME`);
  return binary;
}

export async function waitUntilReady(page) {
  for (let attempt = 1; ; attempt++) {
    try {
      await page.waitForFunction(() => window.gantt?.ready && window.gantt.status().frames >= 30, null, { timeout: 180_000 });
      return;
    } catch (err) {
      if (attempt >= 3 || !/context was destroyed|navigat/i.test(String(err))) throw err;
      await page.waitForLoadState("load");
    }
  }
}

export async function measureFps(page) {
  return page.evaluate(() => new Promise((done) => {
    let started;
    let count = 0;
    const frame = (now) => {
      if (started === undefined) started = now;
      else if (++count === 30) {
        done(30_000 / (now - started));
        return;
      }
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  }));
}

export async function graphicsInfo(page) {
  try {
    return await page.evaluate(async () => {
      let renderer = "unknown";
      try {
        const gl = document.createElement("canvas").getContext("webgl2");
        const ext = gl?.getExtension("WEBGL_debug_renderer_info");
        if (gl && ext) renderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL);
      } catch {}
      let adapter = "unknown";
      try {
        const gpuAdapter = await navigator.gpu?.requestAdapter();
        adapter = gpuAdapter?.info?.description || gpuAdapter?.info?.device || "unknown";
      } catch {}
      return { renderer, adapter };
    });
  } catch {
    return { renderer: "unknown", adapter: "unknown" };
  }
}

// The pre-roll: the page is loaded and the frame rate is measured, and `setup` (steps.mjs) puts the view where the
// video opens. The stamp comes after it, so the cut (record.mjs trims the video to the stamp) never films the set-up.
export async function preRoll(page, fps, setup = script.setup) {
  const ctx = await createContext(page, { mode: "record", fps });
  await setup?.(ctx);
  return { ctx, tReady: Date.now() };
}
