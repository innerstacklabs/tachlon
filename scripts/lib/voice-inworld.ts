// The Inworld text-to-speech voice for narrate.ts (tl#1282), the request shape Neuralingual uses.
// `fetch` and `probeSeconds` are injected so tests make no network call and need no ffprobe.
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Voice } from "./narrate.ts";

export const INWORLD_URL = "https://api.inworld.ai/tts/v1/voice";

export interface InworldOptions {
  /** The Basic auth credential from Inworld's portal; never logged or thrown. */
  key: string;
  voiceId: string;
  modelId?: string;
  fetch?: typeof fetch;
  /** The length of the encoded audio in seconds; `ffprobe` unless given. */
  probeSeconds?: (bytes: Uint8Array) => number;
}

/** The length of an encoded clip, read by `ffprobe` from a temporary file (a pipe has no MP3 duration). */
export function ffprobeSeconds(bytes: Uint8Array): number {
  const dir = mkdtempSync(join(tmpdir(), "voice-inworld-"));
  try {
    writeFileSync(join(dir, "clip.mp3"), bytes);
    const out = execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", join(dir, "clip.mp3")], { encoding: "utf8" });
    const seconds = Number(out.trim());
    if (!Number.isFinite(seconds)) throw new Error(`ffprobe: no duration in ${JSON.stringify(out.trim())}`);
    return seconds;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

export function inworldVoice(opts: InworldOptions): Voice {
  const { key, voiceId, modelId = "inworld-tts-1", fetch: get = fetch, probeSeconds = ffprobeSeconds } = opts;
  return async (text) => {
    const res = await get(INWORLD_URL, {
      method: "POST",
      headers: { Authorization: `Basic ${key}`, "Content-Type": "application/json" },
      body: JSON.stringify({ text, voiceId, modelId, outputFormat: "mp3" }),
    });
    if (!res.ok) throw new Error(`inworld: HTTP ${res.status} for voice ${voiceId}`);
    const { audioContent } = (await res.json()) as { audioContent?: string };
    if (!audioContent) throw new Error(`inworld: HTTP ${res.status} with no audioContent`);
    const bytes = new Uint8Array(Buffer.from(audioContent, "base64"));
    return { bytes, seconds: probeSeconds(bytes) };
  };
}
