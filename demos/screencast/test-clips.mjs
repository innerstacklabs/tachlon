// Checks clip cutting (gs#152, S2) with ffmpeg and no browser: node demos/screencast/test-clips.mjs
// Needs ffmpeg (set FFMPEG, or have it on PATH). It runs on hub; it is not in a CI step, because the CI runner has no ffmpeg yet.
//
// The source is an ffmpeg testsrc video with a time stamp drawn as a black-and-white bar code in its top strip (tenths of
// a second, 8 bits), so a clip says by itself which part of the video it was cut from. Three known windows are cut; the
// third has noise on it, so it does not fit the first rung of the ladder. Then: each clip starts and ends within 0.3 s of
// its window and is under the cap; a clip that fits no rung is not written; an unchanged scene hash copies nothing; a
// changed scene, a bumped look epoch and a new clip copy; a recording that is not WebGPU or is under 50 fps, and a clip over
// the cap, copy nothing; run.sh guards before it copies.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { cutClips, moduleDigest, sceneHash, sceneModuleDigests } from "./record/clips.mjs";
import { clipLimit, clipMaxSeconds, ffmpegPath, makeClip } from "./record/media.mjs";
import { checkRecording, copyClips, planCopy } from "./copy-clips.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const ffmpeg = ffmpegPath();
const work = mkdtempSync(join(tmpdir(), "test-clips-"));
try {
  // --- the source video -------------------------------------------------------------------------------------------
  const source = join(work, "source.mp4");
  const bar = "if(lt(Y,20),255*mod(floor(floor(T*10)/pow(2,floor(X/80))),2),lum(X,Y))";
  const filter = `testsrc=duration=20:size=640x360:rate=30,format=yuv420p,noise=alls=2:allf=t:enable='between(t,12,18.5)',geq=lum='${bar}':cb='cb(X,Y)':cr='cr(X,Y)'`;
  execFileSync(ffmpeg, ["-y", "-v", "error", "-f", "lavfi", "-i", filter, "-c:v", "libx264", "-crf", "10", "-pix_fmt", "yuv420p", source]);

  /** The video time of every frame of a clip, read back from its bar code (0.1 s steps, so +0.05 is the middle of the step). */
  const frameTimes = (clip) => {
    const raw = execFileSync(ffmpeg, ["-v", "error", "-i", clip, "-vf", "scale=640:360:flags=neighbor", "-pix_fmt", "gray", "-f", "rawvideo", "-"], { maxBuffer: 1 << 29 });
    const size = 640 * 360;
    assert.equal(raw.length % size, 0, "whole frames");
    const times = [];
    for (let at = 0; at < raw.length; at += size) {
      let tenths = 0;
      for (let bit = 0; bit < 8; bit++) tenths += (raw[at + 10 * 640 + 40 + 80 * bit] > 127 ? 1 : 0) << bit;
      times.push(tenths / 10 + 0.05);
    }
    return times;
  };

  // --- three windows -> three clips -------------------------------------------------------------------------------
  const run = async () => {};
  const scenes = [
    { name: "alpha", journey: "J2", clip: true, caption: "A", run },
    { name: "story-only", caption: "S", run },
    { name: "beta", journey: "J2", clip: true, caption: "B", run },
    { name: "gamma", journey: "J6", clip: true, caption: "C", run },
  ];
  const digests = new Map(scenes.map((scene) => [scene.name, `digest-${scene.name}`]));
  const tPage = 1_000_000;
  const at = (seconds) => tPage + seconds * 1000;
  const windows = [
    { name: "alpha", clip: true, startMs: at(2), endMs: at(5.5) },
    { name: "story-only", clip: false, startMs: at(6), endMs: at(7.5) }, // no clip: true, so no clip
    { name: "beta", clip: true, startMs: at(8), endMs: at(10) },
    { name: "gamma", clip: true, startMs: at(12), endMs: at(18.5) },
  ];
  const fresh = join(work, "fresh");
  const recording = { backend: "WebGPU", fps: 120, commit: "abc1234", branch: "feat/test" };
  const { clips, failures } = await cutClips({ source, dir: fresh, windows, scenes, digests, tPage, ffmpeg, temp: work, recording });
  assert.deepEqual(failures, []);
  assert.deepEqual(readdirSync(fresh).sort(), ["alpha.gif", "beta.gif", "clips.json", "gamma.gif"], "one GIF per clip: true scene, and the manifest");
  assert.deepEqual(Object.keys(clips), ["alpha", "beta", "gamma"]);
  for (const [name, start, end] of [["alpha", 2, 5.5], ["beta", 8, 10], ["gamma", 12, 18.5]]) {
    const times = frameTimes(join(fresh, `${name}.gif`));
    const frame = 1 / clips[name].fps;
    assert.ok(Math.abs(times[0] - start) <= 0.3, `${name} starts at ${times[0]} s, window starts at ${start} s`);
    assert.ok(Math.abs(times.at(-1) + frame - end) <= 0.3, `${name} ends at ${times.at(-1) + frame} s, window ends at ${end} s`);
    assert.ok(clips[name].bytes < clipLimit, `${name} is ${clips[name].bytes} bytes, the cap is ${clipLimit}`);
    assert.equal(clips[name].bytes, readFileSync(join(fresh, `${name}.gif`)).length, `${name}: the manifest size is the file's size`);
    assert.ok(clips[name].width <= 640 && clips[name].fps <= 8, `${name}: at most 640 px and 8 fps`);
    assert.equal(clips[name].hash, sceneHash(scenes.find((scene) => scene.name === name), digests.get(name)));
  }
  assert.ok(clips.gamma.width < 640 || clips.gamma.fps < 8, `gamma is noisy and must be past the first rung (${clips.gamma.width}px ${clips.gamma.fps} fps)`);
  const manifest = JSON.parse(readFileSync(join(fresh, "clips.json"), "utf8"));
  assert.deepEqual(manifest.recording, recording);

  // --- the cap and the length limit are not suggestions -------------------------------------------------------------
  const lost = join(work, "lost.gif");
  await assert.rejects(() => makeClip(source, lost, 2, 5.5, ffmpeg, work, { limit: 1000 }), /no rung fits 1000 bytes; tried 640px\/8fps=\d+/);
  assert.equal(existsSync(lost), false, "a clip that fits no rung is not written");
  await assert.rejects(() => makeClip(source, lost, 0, clipMaxSeconds + 0.5, ffmpeg, work), /clip window is 12\.5 s; the limit is 12 s/);
  await assert.rejects(() => makeClip(source, lost, 3, 3, ffmpeg, work), /is empty/);
  const tooLong = await cutClips({
    source, dir: join(work, "long"), digests, tPage, ffmpeg, temp: work, recording,
    windows: [{ name: "long", clip: true, startMs: at(0), endMs: at(13) }, { name: "alpha", clip: true, startMs: at(2), endMs: at(4) }],
    scenes,
  });
  assert.deepEqual(tooLong.failures.map((item) => item.name), ["long"], "a scene over 12 s fails alone");
  assert.deepEqual(Object.keys(tooLong.clips), ["alpha"], "the other clip is still cut");
  const badName = await cutClips({ source, dir: join(work, "bad"), digests, tPage, ffmpeg, temp: work, recording, scenes, windows: [{ name: "../x", clip: true, startMs: at(1), endMs: at(2) }] });
  assert.match(badName.failures[0].message, /not a file-name slug/);

  // --- the scene hash -----------------------------------------------------------------------------------------------
  const hashOf = (patch) => sceneHash({ name: "s", journey: "J2", caption: "C", run: async () => 1, ...patch }, "digest");
  assert.equal(hashOf({}), hashOf({}), "the same scene hashes the same");
  assert.notEqual(hashOf({}), hashOf({ run: async () => 2 }), "the actions are in the hash");
  assert.notEqual(hashOf({}), hashOf({ caption: "D" }), "the caption is in the hash");
  assert.notEqual(hashOf({ caption: () => "a" }), hashOf({ caption: () => "b" }), "a function caption is in the hash");
  assert.notEqual(hashOf({}), hashOf({ pre: "true" }), "the pre is in the hash");
  const scene = { name: "s", caption: "C", run };
  assert.notEqual(sceneHash(scene, "a"), sceneHash(scene, "b"), "the digest of the module source is in the hash");
  assert.throws(() => sceneHash(scene), /no digest of its module's source/);

  // --- the hash covers the module that defines the scene and everything it imports --------------------------------------
  // list.mjs holds the scene "linkish" and imports helper.mjs (a function and a constant) and lone.mjs (a scene of its own).
  const fixture = join(work, "fixture");
  mkdirSync(fixture);
  const write = (name, text) => writeFileSync(join(fixture, name), text);
  const helper = (body, plan) => `export const PLAN = ${plan};\nexport async function addLink(ctx) { ${body} }\n`;
  write("helper.mjs", helper("return 1;", "[1, 2]"));
  write("lone.mjs", 'export const lone = { name: "lone", caption: "L", run: async () => 3 };\n');
  const listText = (limit) => `import { PLAN, addLink } from "./helper.mjs";
import { lone } from "./lone.mjs";
export const LIMIT = ${limit};
export const steps = [{ name: "linkish", caption: "K", run: async (ctx) => addLink(ctx, PLAN, LIMIT) }, lone];
`;
  write("list.mjs", listText(5));
  const listFile = join(fixture, "list.mjs");
  const { steps: fixtureScenes } = await import(pathToFileURL(listFile).href);
  const hashes = async () => {
    const found = await sceneModuleDigests(listFile, fixtureScenes);
    return Object.fromEntries(fixtureScenes.map((item) => [item.name, sceneHash(item, found.get(item.name))]));
  };
  const base = await hashes();
  assert.deepEqual(await hashes(), base, "the same source hashes the same");
  const copiedAfter = (before, after) => planCopy(
    { clips: Object.fromEntries(Object.entries(after).map(([name, hash]) => [name, { hash }])) },
    { lookEpoch: 1, clips: Object.fromEntries(Object.entries(before).map(([name, hash]) => [name, { hash, lookEpoch: 1 }])) },
  );

  write("helper.mjs", helper("await ctx.hold(5); return 1;", "[1, 2]")); // a function the scene's run calls
  const afterHelper = await hashes();
  assert.notEqual(afterHelper.linkish, base.linkish, "editing a helper the scene calls changes its hash");
  assert.deepEqual(copiedAfter(base, afterHelper).copy, [{ name: "linkish", why: "scene changed" }], "and the clip is copied");
  assert.deepEqual(copiedAfter(base, afterHelper).unchanged, ["lone"], "a scene whose module does not import the helper keeps its clip");

  write("helper.mjs", helper("await ctx.hold(5); return 1;", "[1, 3]")); // a constant the scene's run reads
  const afterConstant = await hashes();
  assert.notEqual(afterConstant.linkish, afterHelper.linkish, "editing a constant the scene reads changes its hash");
  assert.deepEqual(copiedAfter(afterHelper, afterConstant).copy, [{ name: "linkish", why: "scene changed" }]);

  write("list.mjs", listText(6)); // a constant in the module that defines the scene
  const afterLimit = await hashes();
  assert.notEqual(afterLimit.linkish, afterConstant.linkish, "editing the scene's own module changes its hash");
  assert.equal(afterLimit.lone, afterConstant.lone, "lone.mjs does not import list.mjs, so lone keeps its hash");

  write("lone.mjs", 'export const lone = { name: "lone", caption: "L", run: async () => 3 };\n// edited\n');
  const afterLone = await hashes();
  assert.notEqual(afterLone.lone, afterLimit.lone, "editing the module a scene is exported from changes its hash");
  assert.notEqual(afterLone.linkish, afterLimit.linkish, "a module the list imports is part of the digest of a scene written in the list");

  // The digest does not depend on where the checkout is: the Mac copy of the files hashes like the hub's.
  const moved = join(work, "moved");
  execFileSync("cp", ["-r", fixture, moved]);
  assert.equal(moduleDigest(join(moved, "list.mjs"), moved), moduleDigest(listFile, fixture));

  // --- copy: an unchanged hash copies nothing -----------------------------------------------------------------------
  const repo = join(work, "repo");
  const first = await copyClips(fresh, repo);
  assert.deepEqual(first.copy.map((item) => item.name).sort(), ["alpha", "beta", "gamma"], "new clips are copied");
  assert.deepEqual(readdirSync(repo).sort(), ["alpha.gif", "beta.gif", "clips.json", "gamma.gif"]);
  const committed = JSON.parse(readFileSync(join(repo, "clips.json"), "utf8"));
  assert.equal(committed.lookEpoch, 1);
  assert.equal(committed.clips.alpha.lookEpoch, 1);
  assert.equal(committed.clips.alpha.hash, clips.alpha.hash);

  // A sentinel in each committed clip shows what a second copy touches: only a copied clip loses it.
  for (const name of ["alpha", "beta", "gamma"]) writeFileSync(join(repo, `${name}.gif`), `committed ${name}`);
  const manifestBefore = readFileSync(join(repo, "clips.json"), "utf8");
  const second = await copyClips(fresh, repo);
  assert.deepEqual(second.copy, [], "an unchanged hash copies nothing");
  assert.deepEqual(second.unchanged.sort(), ["alpha", "beta", "gamma"]);
  assert.equal(second.written, false);
  for (const name of ["alpha", "beta", "gamma"]) assert.equal(readFileSync(join(repo, `${name}.gif`), "utf8"), `committed ${name}`, `${name} was not overwritten`);
  assert.equal(readFileSync(join(repo, "clips.json"), "utf8"), manifestBefore, "the manifest is not rewritten");

  // One scene changes: only that clip is copied.
  const changed = JSON.parse(readFileSync(join(fresh, "clips.json"), "utf8"));
  changed.clips.beta.hash = "0".repeat(64);
  const fresh2 = join(work, "fresh2");
  execFileSync("cp", ["-r", fresh, fresh2]);
  writeFileSync(join(fresh2, "clips.json"), JSON.stringify(changed));
  const third = await copyClips(fresh2, repo);
  assert.deepEqual(third.copy, [{ name: "beta", why: "scene changed" }]);
  assert.equal(readFileSync(join(repo, "alpha.gif"), "utf8"), "committed alpha");
  assert.equal(readFileSync(join(repo, "beta.gif")).length, clips.beta.bytes, "the changed clip is the fresh file");
  assert.equal(JSON.parse(readFileSync(join(repo, "clips.json"), "utf8")).clips.beta.hash, "0".repeat(64));

  // The look epoch is bumped by hand in the committed manifest: every clip cut under the old one is copied.
  const bumped = JSON.parse(readFileSync(join(repo, "clips.json"), "utf8"));
  bumped.lookEpoch = 2;
  writeFileSync(join(repo, "clips.json"), JSON.stringify(bumped));
  const fourth = await copyClips(fresh2, repo);
  assert.deepEqual(fourth.copy.map((item) => `${item.name}:${item.why}`).sort(), ["alpha:look epoch", "beta:look epoch", "gamma:look epoch"]);
  assert.equal(JSON.parse(readFileSync(join(repo, "clips.json"), "utf8")).clips.alpha.lookEpoch, 2);
  assert.deepEqual((await copyClips(fresh2, repo)).copy, [], "and then nothing is copied again");

  // A committed clip the recording no longer has is left alone, and a clip that is not committed is copied.
  const only = { recording, clips: { delta: { ...clips.alpha, hash: "1".repeat(64) } } };
  const fresh3 = join(work, "fresh3");
  execFileSync("cp", ["-r", fresh, fresh3]);
  execFileSync("cp", [join(fresh, "alpha.gif"), join(fresh3, "delta.gif")]);
  writeFileSync(join(fresh3, "clips.json"), JSON.stringify(only));
  assert.deepEqual((await copyClips(fresh3, repo)).copy, [{ name: "delta", why: "new" }]);
  assert.deepEqual(Object.keys(JSON.parse(readFileSync(join(repo, "clips.json"), "utf8")).clips), ["alpha", "beta", "delta", "gamma"]);

  // --- nothing is copied from a recording that is not WebGPU at 50 fps or more ----------------------------------------
  checkRecording({ backend: "WebGPU", fps: 50 }, "ok");
  for (const bad of [{ backend: "WebGL2", fps: 120 }, { backend: "WebGPU", fps: 49.9 }, { backend: "WebGPU" }, { fps: 120 }, undefined]) {
    assert.throws(() => checkRecording(bad, "rec"), /rec: (backend|fps)/, JSON.stringify(bad));
  }
  const refused = join(work, "refused");
  for (const bad of [{ backend: "WebGL2", fps: 120 }, { backend: "WebGPU", fps: 6 }]) {
    execFileSync("cp", ["-r", fresh, join(work, "fresh4")]);
    writeFileSync(join(work, "fresh4", "clips.json"), JSON.stringify({ ...JSON.parse(readFileSync(join(fresh, "clips.json"), "utf8")), recording: bad }));
    await assert.rejects(() => copyClips(join(work, "fresh4"), refused), /clips\.json: (backend|fps)/);
    assert.equal(existsSync(refused), false, "a refused recording writes nothing");
    rmSync(join(work, "fresh4"), { recursive: true });
  }
  const guardFile = (name, meta) => {
    const file = join(work, name);
    writeFileSync(file, JSON.stringify(meta));
    return file;
  };
  const guard = (...files) => spawnSync("node", [join(here, "copy-clips.mjs"), "guard", ...files], { encoding: "utf8" });
  assert.equal(guard(guardFile("good.json", { backend: "WebGPU", fps: 120 })).status, 0);
  const draft = guard(guardFile("draft.json", { backend: "WebGL2", fps: 6 }));
  assert.equal(draft.status, 1, "the guard exits non-zero for a draft");
  assert.match(draft.stderr, /backend is "WebGL2"/);
  assert.equal(guard(guardFile("good2.json", { backend: "WebGPU", fps: 120 }), guardFile("slow.json", { backend: "WebGPU", fps: 30 })).status, 1, "every file is checked");
  assert.equal(guard(guardFile("fresh-slow-clips.json", { recording: { backend: "WebGPU", fps: 30 }, clips: {} })).status, 1, "clips.json is checked through its recording");

  // --- a clip over the cap is never copied, and nothing else is copied with it -------------------------------------------
  const fresh5 = join(work, "fresh5");
  const repo5 = join(work, "repo5");
  execFileSync("cp", ["-r", fresh, fresh5]);
  writeFileSync(join(fresh5, "beta.gif"), Buffer.alloc(clipLimit)); // the manifest still says the small size: the file on disk is what counts
  await assert.rejects(() => copyClips(fresh5, repo5), /clip beta: 600000 bytes, not under 600000; nothing was copied/);
  assert.equal(existsSync(repo5), false, "nothing was copied");

  // --- plan, for the record ----------------------------------------------------------------------------------------------
  assert.deepEqual(planCopy({ clips: {} }, null), { copy: [], unchanged: [], lookEpoch: 1 });

  // --- run.sh: guard first, copy before the story media, both lists recorded ---------------------------------------------------
  const runSh = readFileSync(join(here, "run.sh"), "utf8");
  const where = (text) => {
    const index = runSh.indexOf(text);
    assert.ok(index >= 0, `run.sh has: ${text}`);
    return index;
  };
  assert.ok(where("copy-clips.mjs guard") < where("cp \"$stage/screencast.gif\""), "the guard comes before anything is copied");
  assert.match(runSh, /copy-clips\.mjs guard "\$stage\/screencast\.json" "\$stage\/features\/clips\.json"/, "the guard reads both recordings");
  assert.ok(where("copy-clips.mjs copy") < where("cp \"$stage/screencast.gif\""), "the clips are validated before the story media is copied");
  assert.equal(runSh.match(/record\.mjs --list features/g)?.length, 2, "the features list is recorded on the Mac and in the draft");
  assert.match(runSh, /out\/features\//, "the features folder is brought back from the Mac");
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log("test-clips: ok");
