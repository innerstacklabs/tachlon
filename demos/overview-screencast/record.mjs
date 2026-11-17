// Records the overview's scenes (steps.mjs) in one headless Chrome session on the machine that has the GPU, and writes
//   <out>/overview.mp4   the silent video, 1280x720, 30 fps, cut where the first scene starts
//   <out>/overview.json  the machine, backend and measured frame rate, and each scene's window in seconds of that video
// Usage: node record.mjs --voice <dir of the clips scripts/narrate.ts wrote> --out <dir> --url <the public demo address with ?sample=demo-50k> [--draft]
// Each scene lasts at least its line's audio plus LEAD and TAIL, so join.ts can lay the voice over it. Without --draft the
// run refuses, and writes nothing, unless the page reports WebGPU at 50 fps or more (the rule of demos/screencast/README.md).
// Host: the Mac mini (CHROME=<Google Chrome binary>, node with playwright-core beside it); a hub run needs --draft.
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { cpus, platform, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { checkRecording } from "../screencast/copy-clips.mjs";
import { runScenes } from "../screencast/ctx.mjs";
import { chromePath, graphicsInfo, loadChromium, measureFps, preRoll, useSystemFfmpegForVideo, waitUntilReady } from "../screencast/record/browser.mjs";
import { installOverlay } from "../screencast/record/overlay.mjs";
import { execFile, ffmpegPath, stopChildren } from "../screencast/record/media.mjs";
import { LEAD, TAIL, setup, steps } from "./steps.mjs";

const { values: o } = parseArgs({ options: { url: { type: "string" }, voice: { type: "string" }, out: { type: "string" }, draft: { type: "boolean" } } });
if (!o.voice || !o.out || !o.url) {
  console.error("usage: node record.mjs --voice <narrate.ts clips dir> --out <dir> --url <the public demo address with ?sample=demo-50k> [--draft]");
  process.exit(2);
}
// narrate.ts keeps each clip's length in <key>.json beside the audio; the scenes look their line up by text.
const spoken = new Map();
for (const file of (await readdir(o.voice)).filter((name) => /^[0-9a-f]{16}\.json$/.test(name))) {
  const clip = JSON.parse(await readFile(join(o.voice, file), "utf8"));
  spoken.set(clip.text, clip.seconds);
}
const audioSeconds = async (text) => {
  if (!spoken.has(text)) throw new Error(`no spoken clip for "${text}"; run scripts/narrate.ts on narration.txt first`);
  return spoken.get(text) + LEAD + TAIL;
};

const out = resolve(o.out);
const temp = await mkdtemp(join(tmpdir(), "overview-"));
let browser;
try {
  await useSystemFfmpegForVideo(temp);
  const launch = { headless: true, executablePath: await chromePath() };
  if (platform() === "darwin") launch.args = ["--use-angle=metal", "--ignore-gpu-blocklist", "--enable-gpu", "--enable-unsafe-webgpu"];
  browser = await (await loadChromium()).launch(launch);
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, recordVideo: { dir: temp, size: { width: 1280, height: 720 } } });
  await installOverlay(context);
  const page = await context.newPage();
  const tPage = Date.now();
  let failure;
  page.on("pageerror", (err) => { failure ??= err; });
  await page.goto(o.url);
  await waitUntilReady(page);
  const fps = await measureFps(page);
  const { ctx, tReady } = await preRoll(page, fps, setup);
  const { windows } = await runScenes(ctx, steps, { audioSeconds });
  if (failure) throw new Error(`page broke: ${failure.message}`);
  const { backend } = await page.evaluate(() => window.gantt.status());
  const graphics = await graphicsInfo(page);
  const video = page.video();
  await page.close();
  await context.close();
  if (!o.draft) checkRecording({ backend, fps }, "overview");
  const trim = Math.max(0, (tReady - tPage) / 1000 - 0.3);
  const at = (ms) => Math.round(Math.max(0, (ms - tPage) / 1000 - trim) * 1000) / 1000;
  await mkdir(out, { recursive: true });
  await execFile(ffmpegPath(), ["-y", "-i", await video.path(), "-ss", String(trim), "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p", "-r", "30", join(out, "overview.mp4")]);
  const date = new Date().toISOString().slice(0, 10);
  const meta = { machine: cpus()[0]?.model ?? "unknown", date, browser: browser.version(), url: o.url, backend, renderer: graphics.adapter !== "unknown" ? graphics.adapter : graphics.renderer, fps, windows: windows.map((w) => ({ name: w.name, start: at(w.startMs), end: at(w.endMs) })) };
  await writeFile(join(out, "overview.json"), `${JSON.stringify(meta, null, 2)}\n`);
  console.log(`overview: ${windows.length} scenes, ${meta.windows.at(-1).end.toFixed(1)} s, ${backend}, ${fps.toFixed(1)} fps`);
} finally {
  await stopChildren();
  await browser?.close().catch(() => {});
  await rm(temp, { recursive: true, force: true });
}
