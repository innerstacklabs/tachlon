// Joins the overview: speaks narration.txt (scripts/narrate.ts, Inworld; needs INWORLD_API_KEY), lays each line over its
// scene in record.mjs's video and writes the narrated mp4 and its captions. Host: hub (ffmpeg, ffprobe, Node 22.18 or later).
//   node scripts/narrate.ts demos/overview-screencast/narration.txt <voice dir> --voice inworld Tyler   (speak first; record.mjs reads the lengths)
//   node demos/overview-screencast/join.ts <voice dir> <record.mjs's out dir> <media dir>
// Writes <media dir>/overview-narrated.mp4, .vtt and .json (the recording's machine, backend and frame rate).
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { toVtt, type Clip } from "../../scripts/lib/narrate.ts";
import { LEAD } from "./steps.mjs";

export interface Window {
  name: string;
  start: number;
  end: number;
}

/** Each line starts LEAD seconds into its scene's window. A line that would run past the window's end fails, naming the scene. */
export function place(windows: readonly Window[], clips: readonly Clip[]): Clip[] {
  if (windows.length !== clips.length) throw new Error(`${windows.length} scenes but ${clips.length} narration lines`);
  return clips.map((clip, i) => {
    const w = windows[i]!;
    if (w.start + LEAD + clip.seconds > w.end) throw new Error(`scene ${w.name}: its line runs ${clip.seconds.toFixed(2)} s from ${(w.start + LEAD).toFixed(2)} s, past the scene's end at ${w.end.toFixed(2)} s`);
    return { ...clip, start: w.start + LEAD };
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const [voice, rec, media] = process.argv.slice(2);
  if (!voice || !rec || !media) {
    console.error("usage: node join.ts <voice dir> <record.mjs out dir> <media dir>");
    process.exit(2);
  }
  const here = dirname(fileURLToPath(import.meta.url));
  const spoken = execFileSync("node", [join(here, "../../scripts/narrate.ts"), join(here, "narration.txt"), voice, "--voice", "inworld", "Tyler"], { encoding: "utf8" })
    .trim().split("\n").map((l) => /^(?:cached|spoken) (\S+) ([\d.]+)s (.*)$/.exec(l)!);
  const meta = JSON.parse(readFileSync(join(rec, "overview.json"), "utf8")) as { windows: Window[] };
  const placed = place(meta.windows, spoken.map((m) => ({ text: m[3]!, file: m[1]!, seconds: Number(m[2]), start: 0, cached: true })));
  const silent = join(rec, "overview.mp4");
  const length = Number(execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", silent], { encoding: "utf8" }).trim());
  const end = Math.max(length, ...placed.map((p) => p.start + p.seconds)).toFixed(2);
  const delays = placed.map((p, i) => `[${i + 1}]adelay=${Math.round(p.start * 1000)}:all=1[a${i}]`);
  mkdirSync(media, { recursive: true });
  const mp4 = join(media, "overview-narrated.mp4");
  // As setup-screencast/join.mjs: the video is re-encoded and the length set with -t, because apad with -shortest and -c:v copy hangs.
  execFileSync("ffmpeg", ["-y", "-v", "error", "-i", silent, ...placed.flatMap((p) => ["-i", join(voice, p.file)]),
    "-filter_complex", `${delays.join(";")};${placed.map((_, i) => `[a${i}]`).join("")}amix=inputs=${placed.length}:normalize=0:duration=longest,apad=whole_dur=${end}[a]`,
    "-map", "0:v", "-map", "[a]", "-c:v", "libx264", "-preset", "slow", "-crf", "27", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "96k", "-t", end, "-movflags", "+faststart", mp4]);
  writeFileSync(join(media, "overview-narrated.vtt"), toVtt(placed));
  copyFileSync(join(rec, "overview.json"), join(media, "overview-narrated.json"));
  console.log(`${mp4}: ${end} s`);
}
