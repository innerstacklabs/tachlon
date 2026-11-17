// Narration for the screencast (tl#1268): each line of a script becomes an audio file and a WebVTT cue.
// The voice is injected, so this module makes no network call and tests run with a fake. A clip is cached
// under a hash of its text and the voice id: a re-record speaks only the lines that changed.
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** Speaks one line: the encoded audio and its length in seconds. */
export type Voice = (text: string) => Promise<{ bytes: Uint8Array; seconds: number }>;

export interface NarrateOptions {
  voice: Voice;
  /** Names the voice and its settings; a new id misses the cache. */
  voiceId: string;
  /** Where clips, their lengths and `narration.vtt` go. */
  dir: string;
  /** The audio file extension, `mp3` unless the voice says otherwise. */
  ext?: string;
}

export interface Clip {
  text: string;
  file: string;
  start: number;
  seconds: number;
  cached: boolean;
}

/** The cache key of one line: the first 16 hex digits of sha256(voice id, NUL, text). */
export const clipKey = (voiceId: string, text: string): string =>
  createHash("sha256").update(`${voiceId}\0${text}`).digest("hex").slice(0, 16);

/** A WebVTT timestamp, `HH:MM:SS.mmm`. */
export function vttTime(seconds: number): string {
  const ms = Math.round(seconds * 1000);
  const pad = (n: number, w = 2): string => String(n).padStart(w, "0");
  return `${pad(Math.floor(ms / 3_600_000))}:${pad(Math.floor(ms / 60_000) % 60)}:${pad(Math.floor(ms / 1000) % 60)}.${pad(ms % 1000, 3)}`;
}

/** The WebVTT for clips played back to back. */
export const toVtt = (clips: readonly Clip[]): string =>
  ["WEBVTT", ...clips.map((c, i) => `${i + 1}\n${vttTime(c.start)} --> ${vttTime(c.start + c.seconds)}\n${c.text}`)].join("\n\n") + "\n";

/** The script's lines: one cue per non-blank line, trimmed. */
export const scriptLines = (script: string): string[] =>
  script.split("\n").map((l) => l.trim()).filter((l) => l !== "");

/** Speaks each line not already cached, writes the clips and `narration.vtt`, and returns the clips. */
export async function narrate(lines: readonly string[], opts: NarrateOptions): Promise<Clip[]> {
  await mkdir(opts.dir, { recursive: true });
  const clips: Clip[] = [];
  let start = 0;
  for (const text of lines) {
    const key = clipKey(opts.voiceId, text);
    const file = `${key}.${opts.ext ?? "mp3"}`;
    const meta = join(opts.dir, `${key}.json`);
    let seconds: number;
    let cached = true;
    try {
      seconds = (JSON.parse(await readFile(meta, "utf8")) as { seconds: number }).seconds;
      await readFile(join(opts.dir, file));
    } catch {
      const spoken = await opts.voice(text);
      seconds = spoken.seconds;
      cached = false;
      await writeFile(join(opts.dir, file), spoken.bytes);
      await writeFile(meta, JSON.stringify({ seconds, text, voiceId: opts.voiceId }) + "\n");
    }
    clips.push({ text, file, start, seconds, cached });
    start += seconds;
  }
  await writeFile(join(opts.dir, "narration.vtt"), toVtt(clips));
  return clips;
}
