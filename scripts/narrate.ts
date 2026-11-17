// Turns a narration script into audio clips and captions (see scripts/lib/narrate.ts):
//   node scripts/narrate.ts <script.txt> <out-dir> <voice-module.ts>
//   node scripts/narrate.ts <script.txt> <out-dir> --voice inworld [voice-id]
// The voice module's default export is `{ id, ext?, voice }`, `voice` a `Voice`. `--voice inworld` uses
// scripts/lib/voice-inworld.ts with the key from INWORLD_API_KEY (voice id `Ashley` unless given).
// Prints one line per clip, `cached` or `spoken`, and writes `<out-dir>/narration.vtt`.
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { narrate, scriptLines, type Voice } from "./lib/narrate.ts";
import { inworldVoice } from "./lib/voice-inworld.ts";

const [scriptPath, dir, voicePath, voiceName, inworldId = "Ashley"] = process.argv.slice(2);
if (!scriptPath || !dir || !voicePath || (voicePath === "--voice" && voiceName !== "inworld")) {
  console.error("usage: node scripts/narrate.ts <script.txt> <out-dir> (<voice-module.ts> | --voice inworld [voice-id])");
  process.exit(2);
}
let mod: { id: string; ext?: string; voice: Voice };
if (voicePath === "--voice") {
  const key = process.env.INWORLD_API_KEY;
  if (!key) {
    console.error("narrate: --voice inworld needs INWORLD_API_KEY");
    process.exit(2);
  }
  mod = { id: `inworld-tts-1:${inworldId}`, voice: inworldVoice({ key, voiceId: inworldId }) };
} else {
  mod = ((await import(pathToFileURL(resolve(voicePath)).href)) as { default: typeof mod }).default;
}
const { id, ext, voice } = mod;
const clips = await narrate(scriptLines(await readFile(scriptPath, "utf8")), { voice, voiceId: id, dir, ext });
for (const c of clips) console.log(`${c.cached ? "cached" : "spoken"} ${c.file} ${c.seconds.toFixed(2)}s ${c.text}`);
