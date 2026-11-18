// One chapter clip for the book (gs#454): play one step from steps-features.mjs in a headless browser and write
// `<step>.mp4` (1280x720, 30 fps), `<step>.webp` (the poster: the last frame) and the screenshots the step takes
// through `ctx.shot(name, badges)`, each a numbered teal badge on a page element or a point. The media budget per
// chapter (docs/CONTRIBUTING.md, "The book"): a clip of 10 to 20 s under 3 MB, a poster or screenshot under 300 KB.
//
//   node demos/screencast/clip.mjs --step open-file --out book/src/media [--file <schedule>] [--name <base>] [--dist <dir>]
//
// The clip records the hosted build (apps/hosted/dist, built by run.sh or `pnpm build` in apps/hosted) opened on
// ?sample=demo-50k, the build and address the README video uses (hosted-build.mjs, gs#601). --dist names a copy of that
// build, as on the MacBook; a folder without samples/samples.json is the plain web build and is refused.
// Into book/src/media the recording must say backend WebGPU at 50 fps or more (the guard of copy-clips.mjs, gs#654); a run that
// fails it writes nothing. Any other --out folder is a draft and is not checked. Where to record: demos/screencast/README.md.

import { access, mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import { platform, tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { checkRecording } from "./copy-clips.mjs";
import { serveDir } from "./serve.mjs";
import * as features from "./steps-features.mjs";
import { runScenes } from "./ctx.mjs";
import { HOSTED_DIST, HOSTED_MARKER, HOSTED_QUERY } from "./hosted-build.mjs";
import { chromePath, loadChromium, measureFps, preRoll, useSystemFfmpegForVideo, waitUntilReady } from "./record/browser.mjs";
import { hideForShot, installOverlay, showAfterShot } from "./record/overlay.mjs";
import { execFile, ffmpegPath, mp4Duration, stopChildren } from "./record/media.mjs";

const usage = `usage: node clip.mjs --step <name> --out <dir> [--file <schedule>] [--name <file base>] [--stills-only] [--dist <hosted build, default ${HOSTED_DIST}>]`;
/** The default build: the hosted one in this checkout, wherever the command runs from. */
export const DEFAULT_DIST = fileURLToPath(new URL(`../../${HOSTED_DIST}`, import.meta.url));
export const CLIP_SECONDS = { min: 10, max: 20 };
export const CLIP_BYTES = 3_000_000;
export const IMAGE_BYTES = 300_000;

/** The clip-length problem, or undefined: none under --stills-only, where the clip is a by-product and a good set of stills exits 0. */
export function clipLengthProblem(seconds, stillsOnly) {
  if (stillsOnly || (seconds >= CLIP_SECONDS.min && seconds <= CLIP_SECONDS.max)) return undefined;
  return `the clip is ${seconds.toFixed(1)} s; the budget is ${CLIP_SECONDS.min} to ${CLIP_SECONDS.max} s`;
}

export function parseArgs(argv) {
  const allowed = new Set(["--dist", "--step", "--out", "--file", "--name"]);
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    if (flag === "--stills-only") { options.stillsOnly = true; i -= 1; continue; } // no value: only the stills are wanted, so no clip-length check
    const value = argv[i + 1];
    if (!allowed.has(flag) || value === undefined || value.startsWith("--")) throw Object.assign(new Error(usage), { exitCode: 2 });
    options[flag.slice(2)] = value;
  }
  if (!options.step || !options.out) throw Object.assign(new Error(usage), { exitCode: 2 });
  options.dist ??= DEFAULT_DIST;
  options.name ??= options.step;
  if (basename(options.name) !== options.name || options.name === "." || options.name === "..") {
    throw Object.assign(new Error(`${usage}\n--name must be a file base`), { exitCode: 2 });
  }
  return options;
}

/** Refuse a build that is not the hosted app: the plain web build has no `samples/` and would ignore `?sample=demo-50k`. */
export async function checkHostedDist(dist) {
  try {
    await access(join(resolve(dist), HOSTED_MARKER));
  } catch {
    throw Object.assign(new Error(`${dist} is not the hosted build: it has no ${HOSTED_MARKER}. Build it with run.sh, or \`pnpm build\` in apps/hosted after \`node scripts/build-samples.mjs\`; clip.mjs records ${HOSTED_DIST}`), { exitCode: 2 });
  }
}

/** True when `out` is the book's media folder or inside it, in any checkout (the path has `book/src/media` in it). */
export function writesToBookMedia(out) {
  const parts = resolve(out).split(sep);
  return parts.some((part, i) => part === "book" && parts[i + 1] === "src" && parts[i + 2] === "media");
}

/** Refuse a recording that is not WebGPU at 50 fps or more when the clip goes into the book; any other folder is a draft. */
export function guardClip(out, recording, step) {
  if (!writesToBookMedia(out)) return;
  try {
    checkRecording(recording, `clip ${step}`);
  } catch (err) {
    throw new Error(`${err.message}. Record motion on the MacBook, or write to a folder outside book/src/media (demos/screencast/README.md); nothing was written`);
  }
}

/** The badges drawn on the page for a screenshot: a numbered teal circle beside an element (right, or `side: "left"`) or at a point. */
export function drawBadges(badges) {
  const drawn = [];
  for (const badge of badges) {
    let x;
    let y;
    if (badge.selector) {
      const box = document.querySelector(badge.selector)?.getBoundingClientRect();
      if (!box || box.width === 0) throw new Error(`badge ${badge.n}: nothing visible matches ${badge.selector}`);
      x = badge.side === "left" ? box.left - 18 : box.right + 18;
      y = box.top + box.height / 2;
    } else {
      x = badge.point.x;
      y = badge.point.y;
    }
    const el = document.createElement("div");
    el.className = "sc-badge";
    el.textContent = String(badge.n);
    Object.assign(el.style, {
      position: "fixed", left: `${x}px`, top: `${y}px`, transform: "translate(-50%, -50%)",
      width: "30px", height: "30px", borderRadius: "50%", background: "#1f7a70", color: "white",
      font: "700 17px/30px system-ui, sans-serif", textAlign: "center", zIndex: "2147483647",
      boxShadow: "0 0 0 3px white, 0 2px 6px rgba(0,0,0,0.35)", pointerEvents: "none",
    });
    document.body.append(el);
    drawn.push(el);
  }
  return drawn.length;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const step = features.steps.find((item) => item.name === options.step);
  if (!step) throw Object.assign(new Error(`no step "${options.step}" in steps-features.mjs; the steps are ${features.steps.map((s) => s.name).join(", ")}`), { exitCode: 2 });
  const out = resolve(options.out);
  await checkHostedDist(options.dist);
  const temp = await mkdtemp(join(tmpdir(), "gantt-clip-"));
  const ffmpeg = ffmpegPath();
  const shots = [];
  let browser;
  let context;
  let server;
  const watchdog = setTimeout(() => { console.error("clip timed out after 10 minutes"); process.exit(1); }, 10 * 60_000);
  watchdog.unref();
  try {
    server = await serveDir(resolve(options.dist));
    await useSystemFfmpegForVideo(temp);
    const chromium = await loadChromium();
    const launch = { headless: true, executablePath: await chromePath() };
    if (platform() === "darwin") launch.args = ["--use-angle=metal", "--ignore-gpu-blocklist", "--enable-gpu"];
    browser = await chromium.launch(launch);
    context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, recordVideo: { dir: temp, size: { width: 1280, height: 720 } } });
    await installOverlay(context);
    const page = await context.newPage();
    const video = page.video();
    const tPage = Date.now();
    let pageFailure;
    page.on("pageerror", (err) => { pageFailure ??= err; });
    await page.goto(`http://127.0.0.1:${server.port}/${HOSTED_QUERY}`);
    await waitUntilReady(page);
    const fps = await measureFps(page);
    const { ctx, tReady } = await preRoll(page, fps, features.setup);
    if (options.file) ctx.file = resolve(options.file);
    // A screenshot: the first-screen line dismissed, the caption and the cursor ring hidden, the badges drawn, the frame captured as PNG, then all undone.
    ctx.shot = async (name, badges) => {
      await hideForShot(page);
      await page.evaluate(drawBadges, badges);
      const png = join(temp, `${name}.png`);
      await page.screenshot({ path: png });
      await page.evaluate(() => { for (const el of document.querySelectorAll(".sc-badge")) el.remove(); });
      await showAfterShot(page);
      shots.push({ name, png });
    };
    await runScenes(ctx, [step]);
    if (pageFailure) throw new Error(`page broke: ${pageFailure.message}`);
    // The backend is read after the scene, as record.mjs does: a WebGPU page that loses its device switches to WebGL2 in place.
    guardClip(out, writesToBookMedia(out) ? { backend: (await page.evaluate(() => window.gantt.status())).backend, fps } : undefined, options.step);
    const recordingEnd = Date.now();
    await page.close();
    await context.close();
    context = undefined;
    const source = await video.path();
    await browser.close();
    browser = undefined;
    await server.close();
    server = undefined;

    await mkdir(out, { recursive: true });
    const trim = Math.max(0, (tReady - tPage) / 1000 - 0.3);
    const mp4 = join(out, `${options.name}.mp4`);
    await execFile(ffmpeg, ["-y", "-i", source, "-ss", String(trim), "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "23", "-pix_fmt", "yuv420p", "-vf", "scale=1280:720:flags=lanczos", "-r", "30", "-movflags", "+faststart", mp4]);
    const seconds = await mp4Duration(ffmpeg, mp4, (recordingEnd - tPage) / 1000 - trim);
    const poster = join(out, `${options.name}.webp`);
    await execFile(ffmpeg, ["-y", "-sseof", "-0.2", "-i", mp4, "-frames:v", "1", "-c:v", "libwebp", "-quality", "85", poster]);
    const files = [{ file: poster, limit: IMAGE_BYTES }];
    for (const shot of shots) {
      const webp = join(out, `${shot.name}.webp`);
      await execFile(ffmpeg, ["-y", "-i", shot.png, "-c:v", "libwebp", "-quality", "85", webp]);
      files.push({ file: webp, limit: IMAGE_BYTES });
    }
    const mp4Bytes = (await stat(mp4)).size;
    const problems = [];
    const lengthProblem = clipLengthProblem(seconds, options.stillsOnly);
    if (lengthProblem) problems.push(lengthProblem);
    if (mp4Bytes > CLIP_BYTES) problems.push(`${basename(mp4)} is ${mp4Bytes} bytes; the budget is under ${CLIP_BYTES}`);
    for (const { file, limit } of files) {
      const bytes = (await stat(file)).size;
      if (bytes > limit) problems.push(`${basename(file)} is ${bytes} bytes; the budget is under ${limit}`);
    }
    console.log(`clip ${options.name}: ${seconds.toFixed(1)} s, ${mp4Bytes} bytes, ${files.length} image${files.length === 1 ? "" : "s"} in ${out}`);
    if (problems.length > 0) {
      for (const problem of problems) console.error(problem);
      return 3;
    }
    return 0;
  } finally {
    clearTimeout(watchdog);
    await stopChildren();
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    await server?.close().catch(() => {});
    await rm(temp, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  try {
    process.exitCode = await main();
  } catch (err) {
    console.error(err?.message ?? err);
    process.exitCode = err?.exitCode ?? 1;
  }
}
