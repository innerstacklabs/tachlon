// Hub only (run.sh): the two checks between a Mac recording and the repo, and the clip copy (gs#152, S2).
//
//   node copy-clips.mjs guard <file.json>...        refuse a recording that is not WebGPU or is under 50 fps
//   node copy-clips.mjs copy <fresh dir> <repo dir> copy the clips whose scene hash or the look epoch changed
//
// The repo dir holds clips.json: { "lookEpoch": N, "clips": { "<scene>": { hash, lookEpoch, bytes, width, fps, seconds } } }.
// A clip is copied only when its scene's hash differs from the committed one, or it was cut under an older lookEpoch,
// or it is not committed yet. ffmpeg output is not byte-stable, so a clip copied for no reason is a new blob in history.
// Bump `lookEpoch` by hand when the chart looks different and every clip should be recut.
import { copyFile, mkdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { clipLimit } from "./record/media.mjs";
import { sceneNamePattern } from "./record/clips.mjs";

export const minFps = 50;

/** A recording's metadata (screencast.json, or `recording` in the fresh clips.json) must say WebGPU and at least 50 fps. */
export function checkRecording(meta, label) {
  const recording = meta ?? {};
  if (recording.backend !== "WebGPU") throw new Error(`${label}: backend is ${JSON.stringify(recording.backend)}, not "WebGPU"; only a WebGPU recording on the Mac is copied into the repo`);
  if (!(Number(recording.fps) >= minFps)) throw new Error(`${label}: fps is ${JSON.stringify(recording.fps)}, under ${minFps}; only a recording at ${minFps} fps or more is copied into the repo`);
}

/** Which fresh clips to copy, and why. `committed` is the repo's clips.json (or null when there is none). */
export function planCopy(fresh, committed) {
  const lookEpoch = committed?.lookEpoch ?? 1;
  const known = committed?.clips ?? {};
  const copy = [];
  const unchanged = [];
  for (const [name, entry] of Object.entries(fresh.clips ?? {})) {
    const old = known[name];
    if (old === undefined) copy.push({ name, why: "new" });
    else if (old.hash !== entry.hash) copy.push({ name, why: "scene changed" });
    else if (old.lookEpoch !== lookEpoch) copy.push({ name, why: "look epoch" });
    else unchanged.push(name);
  }
  return { copy, unchanged, lookEpoch };
}

async function readJson(file, missing) {
  try {
    return JSON.parse(await readFile(file, "utf8"));
  } catch (err) {
    if (err.code === "ENOENT" && missing !== undefined) return missing;
    throw new Error(`${file}: ${err.message}`);
  }
}

/**
 * Copy what changed from `from` to `to` and rewrite `to/clips.json`. Everything is checked before anything is written: the
 * recording guard, each name, and the size of each GIF as it is on disk (not as the manifest says), so a clip over the cap
 * is never copied.
 */
export async function copyClips(from, to) {
  const fresh = await readJson(join(from, "clips.json"));
  checkRecording(fresh.recording, join(from, "clips.json"));
  const committed = await readJson(join(to, "clips.json"), null);
  const plan = planCopy(fresh, committed);
  for (const { name } of plan.copy) {
    if (!sceneNamePattern.test(name)) throw new Error(`clip "${name}": not a file-name slug`);
    const bytes = (await stat(join(from, `${name}.gif`))).size;
    if (!(bytes < clipLimit)) throw new Error(`clip ${name}: ${bytes} bytes, not under ${clipLimit}; nothing was copied`);
  }
  if (plan.copy.length === 0) return { ...plan, written: false };
  await mkdir(to, { recursive: true });
  const clips = { ...(committed?.clips ?? {}) };
  for (const { name } of plan.copy) {
    await copyFile(join(from, `${name}.gif`), join(to, `${name}.gif`));
    clips[name] = { ...fresh.clips[name], lookEpoch: plan.lookEpoch };
  }
  const sorted = Object.fromEntries(Object.keys(clips).sort().map((name) => [name, clips[name]]));
  const next = join(to, "clips.json.tmp");
  await writeFile(next, `${JSON.stringify({ lookEpoch: plan.lookEpoch, clips: sorted }, null, 2)}\n`);
  await rename(next, join(to, "clips.json"));
  return { ...plan, written: true };
}

async function main(argv) {
  const [command, ...args] = argv;
  if (command === "guard" && args.length > 0) {
    for (const file of args) {
      const meta = await readJson(file);
      checkRecording(file.endsWith("clips.json") ? meta.recording : meta, file);
    }
    console.log(`guard: ${args.length} recording(s) are WebGPU at ${minFps} fps or more`);
    return;
  }
  if (command === "copy" && args.length === 2) {
    const result = await copyClips(args[0], args[1]);
    const names = (list) => (list.length ? list.join(", ") : "none");
    console.log(`clips: copied ${result.copy.length} (${names(result.copy.map((item) => `${item.name}: ${item.why}`))}); unchanged ${result.unchanged.length} (${names(result.unchanged)})`);
    return;
  }
  throw new Error("usage: copy-clips.mjs guard <json>... | copy <fresh dir> <repo dir>");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main(process.argv.slice(2));
  } catch (err) {
    console.error(`copy-clips: ${err.message}`);
    process.exitCode = 1;
  }
}
