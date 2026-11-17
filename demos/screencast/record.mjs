import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { cpus, hostname, platform, tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { serveDir } from "./serve.mjs";
import { chromePath, graphicsInfo, loadChromium, measureFps, preRoll, useSystemFfmpegForVideo, waitUntilReady } from "./record/browser.mjs";
import { installOverlay } from "./record/overlay.mjs";
import { runScenes } from "./ctx.mjs";
import * as story from "./steps.mjs";
import * as features from "./steps-features.mjs";
import { ffmpegPath, makeMedia, mp4Duration, stopChildren } from "./record/media.mjs";
import { cutClips, sceneModuleDigests } from "./record/clips.mjs";

const usage = "usage: node record.mjs --dist <dir> --out <dir> [--machine <label>] [--commit <sha>] [--branch <name>] [--date <YYYY-MM-DD>] [--name <file base>] [--list story|features]";
// story: steps.mjs, the README screencast (<name>.gif, .mp4, .json). features: steps-features.mjs, cut into one clip per `clip: true`
// scene in <out>/features/ (gs#152); it makes no GIF or MP4 of its own.
const lists = { story, features };

function parseArgs(argv) {
  const allowed = new Set(["--dist", "--out", "--machine", "--commit", "--branch", "--date", "--name", "--list"]);
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (!allowed.has(flag) || value === undefined || value.startsWith("--")) throw Object.assign(new Error(usage), { exitCode: 2 });
    options[flag.slice(2)] = value;
  }
  if (!options.dist || !options.out) throw Object.assign(new Error(usage), { exitCode: 2 });
  options.list ??= "story";
  if (!Object.hasOwn(lists, options.list)) throw Object.assign(new Error(`${usage}\n--list must be story or features`), { exitCode: 2 });
  options.name ??= "screencast";
  if (!options.name || basename(options.name) !== options.name || options.name === "." || options.name === "..") {
    throw Object.assign(new Error(`${usage}\n--name must be a file base`), { exitCode: 2 });
  }
  if (options.date && !/^\d{4}-\d{2}-\d{2}$/.test(options.date)) {
    throw Object.assign(new Error(`${usage}\n--date must be YYYY-MM-DD`), { exitCode: 2 });
  }
  return options;
}
function localDate() {
  const now = new Date();
  const local = new Date(now.getTime() - now.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 10);
}

// The story must run 45 s or less (gs#384); Narration needs each step's window in the cut.
export const maxSeconds = 45;
export function checkDuration(seconds) {
  return seconds > maxSeconds ? `the screencast runs ${seconds.toFixed(1)} s, over the ${maxSeconds} s limit` : null;
}
// Each scene's start and end in seconds from the cut: `tPage` is when the page opened (ms), `trim` the seconds cut from the front.
export function stepWindows(windows, tPage, trim) {
  const at = (ms) => Math.round(Math.max(0, (ms - tPage) / 1000 - trim) * 1000) / 1000;
  return windows.map(({ name, startMs, endMs }) => ({ name, startSeconds: at(startMs), endSeconds: at(endMs) }));
}

function mb(bytes) {
  return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const out = resolve(options.out);
  const { steps, setup } = lists[options.list];
  const clipping = options.list === "features";
  // A clip is named for its scene and the clips come from the feature list only, so a flag in the story would cut nothing.
  const strayClips = clipping ? [] : steps.filter((scene) => scene.clip).map((scene) => scene.name);
  if (strayClips.length > 0) throw new Error(`steps.mjs scenes set clip: ${strayClips.join(", ")}; clips are cut from steps-features.mjs`);
  await mkdir(out, { recursive: true });
  if (clipping) await rm(join(out, "features"), { recursive: true, force: true });
  else for (const extension of ["gif", "mp4", "json"]) await rm(join(out, `${options.name}.${extension}`), { force: true });
  const temp = await mkdtemp(join(tmpdir(), "gantt-screencast-"));
  let browser;
  let context;
  let server;
  let stopping = false;
  const cleanup = async () => {
    try {
      await stopChildren();
    } finally {
      try {
        await context?.close().catch(() => {});
      } finally {
        context = undefined;
        try {
          await browser?.close().catch(() => {});
        } finally {
          browser = undefined;
          await server?.close().catch(() => {});
          server = undefined;
        }
      }
    }
  };
  const stop = (code, message) => async () => {
    if (stopping) return;
    stopping = true;
    console.error(message);
    setTimeout(() => process.exit(code), 15_000).unref(); // a hung browser.close() must not keep this process alive
    await cleanup();
    await rm(temp, { recursive: true, force: true });
    process.exit(code);
  };
  const interrupt = stop(130, "interrupted");
  const terminate = stop(143, "terminated");
  process.on("SIGINT", interrupt);
  process.on("SIGTERM", terminate);
  const watchdog = setTimeout(stop(1, "screencast timed out after 10 minutes"), 10 * 60_000);
  watchdog.unref();

  try {
    server = await serveDir(resolve(options.dist));
    await useSystemFfmpegForVideo(temp);
    const chromium = await loadChromium();
    const launch = { headless: true, executablePath: await chromePath(), handleSIGINT: false, handleSIGTERM: false, handleSIGHUP: false };
    if (platform() === "darwin") launch.args = ["--use-angle=metal", "--ignore-gpu-blocklist", "--enable-gpu"];
    browser = await chromium.launch(launch);
    const browserVersion = browser.version();
    context = await browser.newContext({
      viewport: { width: 1280, height: 720 },
      deviceScaleFactor: 1,
      recordVideo: { dir: temp, size: { width: 1280, height: 720 } },
    });
    await installOverlay(context);
    const page = await context.newPage();
    const video = page.video();
    const tPage = Date.now();
    let pageFailure;
    page.on("pageerror", (err) => { pageFailure ??= err; });
    page.on("crash", () => { pageFailure ??= new Error("page crashed"); });
    // The hosted build (apps/hosted/dist, gs#541), opened on the built-in 50,000-task sample the scenes name their tasks in.
    await page.goto(`http://127.0.0.1:${server.port}/?sample=demo-50k`);
    await waitUntilReady(page);
    const fps = await measureFps(page);
    const { ctx, tReady } = await preRoll(page, fps, setup); // the stamp comes after the set-up: the video starts on the opening view
    const journey = await runScenes(ctx, steps);
    if (pageFailure) throw new Error(`page broke: ${pageFailure.message}`);
    const finalStatus = await page.evaluate(() => window.gantt.status());
    const graphics = await graphicsInfo(page);
    const recordingEnd = Date.now();
    await page.close();
    await context.close();
    context = undefined;
    const source = await video.path();
    await browser.close();
    browser = undefined;
    await server.close();
    server = undefined;

    const ffmpeg = ffmpegPath();
    const cpu = cpus()[0]?.model ?? "unknown";
    if (clipping) {
      const recording = {
        machine: options.machine ?? `${hostname()}, ${cpu}`,
        commit: options.commit ?? "unknown",
        branch: options.branch ?? "unknown",
        backend: finalStatus.backend,
        fps,
      };
      const { clips, failures } = await cutClips({ source, dir: join(out, "features"), windows: journey.windows, scenes: steps, digests: await sceneModuleDigests(new URL("./steps-features.mjs", import.meta.url).pathname, steps), tPage, ffmpeg, temp, recording });
      if (failures.length > 0) {
        console.error(`${failures.length} clip(s) not cut: ${failures.map((item) => item.name).join(", ")}`);
        return 3;
      }
      console.log(`clips: ${Object.keys(clips).length} cut, ${finalStatus.backend}, ${fps} fps`);
      return 0;
    }
    const trim = Math.max(0, (tReady - tPage) / 1000 - 0.3);
    const media = await makeMedia(source, out, options.name, trim, ffmpeg, temp);
    const durationSeconds = await mp4Duration(ffmpeg, join(out, `${options.name}.mp4`), Math.max(0, (recordingEnd - tPage) / 1000 - trim));
    const metadata = {
      machine: options.machine ?? `${hostname()}, ${cpu}`,
      cpu,
      date: options.date ?? localDate(),
      commit: options.commit ?? "unknown",
      branch: options.branch ?? "unknown",
      backend: finalStatus.backend,
      renderer: finalStatus.backend === "WebGPU" && graphics.adapter !== "unknown" ? graphics.adapter : graphics.renderer,
      browser: browserVersion,
      viewport: "1280x720",
      fps,
      durationSeconds,
      steps: journey.run,
      stepWindows: stepWindows(journey.windows, tPage, trim),
      gif: { bytes: media.gif.bytes, width: media.gif.width, fps: media.gif.fps },
      mp4: media.mp4,
    };
    await writeFile(join(out, `${options.name}.json`), `${JSON.stringify(metadata, null, 2)}\n`);
    if (!media.accepted) {
      await rm(join(out, `${options.name}.gif`), { force: true });
      console.error(`GIF is still ${media.gif.bytes} bytes; sizes tried: ${media.tried.map((item) => `${item.width}px/${item.fps}fps=${item.bytes}`).join(", ")}`);
      return 3;
    }
    const tooLong = checkDuration(durationSeconds);
    if (tooLong) {
      for (const extension of ["gif", "mp4"]) await rm(join(out, `${options.name}.${extension}`), { force: true });
      console.error(tooLong);
      return 3;
    }
    console.log(`screencast: gif ${mb(media.gif.bytes)} (${media.gif.width} px, ${media.gif.fps} fps), mp4 ${mb(media.mp4.bytes)}, ${durationSeconds.toFixed(1)} s, ${finalStatus.backend}`);
    return 0;
  } finally {
    clearTimeout(watchdog);
    process.off("SIGINT", interrupt);
    process.off("SIGTERM", terminate);
    await cleanup();
    await rm(temp, { recursive: true, force: true });
  }
}

// Run as a script; test-frame.mjs imports the two helpers above and must not start a recording.
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    process.exitCode = await main();
  } catch (err) {
    console.error(err?.message ?? err);
    process.exitCode = err?.exitCode ?? 1;
  }
}
