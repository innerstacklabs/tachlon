// Feature clips (gs#152, S2): one GIF per scene that sets `clip: true`, cut out of the one recorded video, and the
// manifest the copy step reads (copy-clips.mjs). Runs on the machine that records: no browser here, only ffmpeg.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { pathToFileURL } from "node:url";
import { importClosure } from "./imports.mjs";
import { makeClip } from "./media.mjs";

/** A clip's file name is its scene name, so a name that is not a plain slug is refused before any path is built. */
export const sceneNamePattern = /^[a-z0-9][a-z0-9-]*$/;

/**
 * What a clip depends on: the scene's own text and `moduleDigest`, the digest of the module that defines it and everything
 * that module imports (sceneModuleDigests). Functions hash as their source, so editing a scene's actions, caption, caption
 * ending or `pre` changes the hash, and so does editing a helper or a constant the scene reads. What is not in the hash: the
 * state the scenes before it leave in the page and the `setup` pre-roll; `lookEpoch` in the committed manifest
 * (copy-clips.mjs) is bumped by hand for those and for a change in how the chart looks.
 */
export function sceneHash(scene, moduleDigest) {
  if (typeof moduleDigest !== "string" || moduleDigest === "") throw new Error(`scene ${scene.name}: no digest of its module's source`);
  const text = (value) => (typeof value === "function" ? String(value) : (value ?? null));
  const parts = [scene.name, scene.journey ?? null, text(scene.pre), text(scene.caption), text(scene.captionEnd), text(scene.run), moduleDigest];
  return createHash("sha256").update(JSON.stringify(parts)).digest("hex");
}

/** The digest of a module's source and of every file it imports, transitively; paths are relative to `root` so it is the same on every machine. */
export function moduleDigest(entry, root) {
  const hash = createHash("sha256");
  for (const file of importClosure(entry).sort()) hash.update(`${relative(root, file)}\0${readFileSync(file, "utf8")}\0`);
  return hash.digest("hex");
}

/**
 * The digest each scene of a list is hashed with: of the module that defines it (the one that exports the scene, or an array
 * that holds it) or, for a scene written inside the list module, of the list module itself. An edit to a shared helper
 * therefore recuts every scene of every module that imports it: more clips copied, never a stale one.
 */
export async function sceneModuleDigests(listFile, scenes) {
  const files = importClosure(listFile);
  const modules = await Promise.all(files.map(async (file) => [file, await import(pathToFileURL(file).href)]));
  const holds = (value, scene) => value === scene || (Array.isArray(value) && value.includes(scene));
  const root = dirname(listFile);
  const digests = new Map();
  for (const scene of scenes) {
    const owner = modules.find(([file, exports]) => file !== listFile && Object.values(exports).some((value) => holds(value, scene)));
    digests.set(scene.name, moduleDigest(owner ? owner[0] : listFile, root));
  }
  return digests;
}

/**
 * The windows of the `clip: true` scenes as seconds into the recorded video. The video starts when the page is created,
 * which is `tPage`; record.mjs trims the full video from the same stamp.
 */
export function clipWindows(windows, tPage) {
  return windows
    .filter((window) => window.clip)
    .map((window) => ({ name: window.name, start: Math.max(0, (window.startMs - tPage) / 1000), end: (window.endMs - tPage) / 1000 }));
}

/**
 * Cut every window to `<dir>/<scene>.gif` and write `<dir>/clips.json`: the recording's backend and frame rate (the guard
 * in run.sh reads them) and, per clip, the scene hash and the size that was written. A scene that cannot be cut does not stop
 * the others; the caller gets the failures and exits non-zero.
 */
export async function cutClips({ source, dir, windows, scenes, digests, tPage, ffmpeg, temp, recording }) {
  await rm(dir, { recursive: true, force: true });
  await mkdir(dir, { recursive: true });
  const byName = new Map(scenes.map((scene) => [scene.name, scene]));
  const clips = {};
  const failures = [];
  for (const window of clipWindows(windows, tPage)) {
    try {
      if (!sceneNamePattern.test(window.name)) throw new Error(`the scene name is not a file-name slug (${sceneNamePattern})`);
      const made = await makeClip(source, join(dir, `${window.name}.gif`), window.start, window.end, ffmpeg, temp);
      clips[window.name] = { hash: sceneHash(byName.get(window.name), digests.get(window.name)), bytes: made.bytes, width: made.width, fps: made.fps, seconds: made.seconds };
      console.log(`clip ${window.name}: ${made.bytes} bytes, ${made.width}px, ${made.fps} fps, ${made.seconds} s`);
    } catch (err) {
      failures.push({ name: window.name, message: err.message });
      console.error(`clip ${window.name}: ${err.message}`);
    }
  }
  await writeFile(join(dir, "clips.json"), `${JSON.stringify({ recording, clips }, null, 2)}\n`);
  return { clips, failures };
}
