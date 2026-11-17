// Checks narration timing with no browser: node demos/screencast/test-narration.mjs
// A step with `say` lasts at least its audio (the injected `audioSeconds`); a step without one, or no `audioSeconds`, is not held.
import assert from "node:assert/strict";
import { runScenes } from "./ctx.mjs";

let clock = 0; // ms; a scene's run takes `ms`, and a hold moves the clock by whole 60 fps frames, as the recorder's does.
const ctx = { frames: async () => {}, setCaption: async () => {}, hold: async (s) => { clock += Math.max(1, Math.round(s * 60)) * (1000 / 60); } };
const scene = (name, ms, say) => ({ name, caption: name, say, run: async () => { clock += ms; } });
const spans = async (list, audioSeconds) => (await runScenes(ctx, list, { now: () => clock, audioSeconds })).windows.map((w) => Math.round(w.endMs - w.startMs));
const asked = [];
const audioSeconds = async (text) => { asked.push(text); return { "It opens": 3, "One drag": 2 }[text]; };
const list = [scene("open", 1000, "It opens"), scene("pan", 500), scene("drag", 4000, "One drag")];

// The say step waits for its audio; a step with no say, or longer than its audio, is not held; no audioSeconds holds nothing.
assert.deepEqual(await spans(list, audioSeconds), [3000, 500, 4000], "each say step's window is at least its audio");
assert.deepEqual(asked, ["It opens", "One drag"], "audioSeconds is asked for each say line, in order");
assert.deepEqual(await spans(list), [1000, 500, 4000], "with no audioSeconds nothing waits (the journey spec)");
// A blank say line, and an audio length that is not a number, fail by the scene's name.
await assert.rejects(() => spans([scene("blank", 10, " ")], audioSeconds), /scene blank failed/);
await assert.rejects(() => spans([scene("odd", 10, "unknown")], audioSeconds), (err) => /audioSeconds gave undefined/.test(err.cause.message));
console.log("test-narration: ok");
