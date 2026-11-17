// Trims each raw clip to its logged start..end, joins them, speaks the spec.json lines with scripts/narrate.ts
// (Inworld; needs INWORLD_API_KEY) and mixes each line in at its segment's start.
// Usage: node join.mjs <work dir> <out.mp4> <poster.webp>
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const [work, out, poster] = process.argv.slice(2);
const clips = join(work, "clips");
const marks = JSON.parse(readFileSync(join(clips, "marks.json"), "utf8"));
const probe = (f) => Number(execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", f]).toString().trim());
const ffmpeg = (...args) => execFileSync("ffmpeg", ["-y", "-v", "error", ...args]);

let at = 0;
const start = {};
const list = [];
for (const seg of Object.keys(marks).sort()) {
  const m = marks[seg];
  const mp4 = join(clips, `${seg}.mp4`);
  ffmpeg("-ss", String(m.start), "-to", String(Math.min(m.end, probe(join(clips, "raw", `${seg}.webm`)))), "-i", join(clips, "raw", `${seg}.webm`),
    "-vf", "fps=30,scale=1280:720:flags=lanczos,format=yuv420p", "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-an", mp4);
  start[seg] = at;
  at += probe(mp4);
  list.push(`file '${mp4}'`);
}
writeFileSync(join(clips, "list.txt"), list.join("\n") + "\n");
const silent = join(clips, "silent.mp4");
ffmpeg("-f", "concat", "-safe", "0", "-i", join(clips, "list.txt"), "-c", "copy", silent);

const spec = JSON.parse(readFileSync(join(import.meta.dirname, "spec.json"), "utf8"));
const say = spec.map((l) => l[2].replaceAll("{pkg}", process.env.PKG));
writeFileSync(join(work, "narration.txt"), say.join("\n") + "\n");
const spoken = execFileSync("node", [join(import.meta.dirname, "../../scripts/narrate.ts"), join(work, "narration.txt"), join(work, "voice"), "--voice", "inworld", "Tyler"], { encoding: "utf8" })
  .trim().split("\n").map((l) => /^(?:cached|spoken) (\S+) ([\d.]+)s /.exec(l));
const placed = spec.map(([seg, offset], i) => ({ file: join(work, "voice", spoken[i][1]), at: start[seg] + offset, seconds: Number(spoken[i][2]) }));
placed.forEach((p, i) => console.log(`line ${i + 1}: ${p.at.toFixed(1)}s, ${p.seconds.toFixed(1)}s long, ${i + 1 < placed.length ? (placed[i + 1].at - p.at - p.seconds).toFixed(1) : "-"}s before the next`));
const end = Math.max(at, ...placed.map((p) => p.at + p.seconds)).toFixed(2);
// Plain apad with -shortest and -c:v copy hangs, so the video is re-encoded and the length is set with -t.
const delays = placed.map((p, i) => `[${i + 1}]adelay=${Math.round(p.at * 1000)}:all=1[a${i}]`);
const mix = placed.map((_, i) => `[a${i}]`).join("");
ffmpeg("-i", silent, ...placed.flatMap((p) => ["-i", p.file]),
  "-filter_complex", `${delays.join(";")};${mix}amix=inputs=${placed.length}:normalize=0:duration=longest,apad=whole_dur=${end}[a]`,
  "-map", "0:v", "-map", "[a]", "-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-c:a", "aac", "-b:a", "128k", "-t", end, "-movflags", "+faststart", out);
// The poster: a frame of the opened office fit-out schedule, 4 s after it is drawn.
const drawn = marks["4b-xer"].steps.find((s) => s.label === "xer drawn").t - marks["4b-xer"].start;
ffmpeg("-ss", String(start["4b-xer"] + drawn + 4), "-i", silent, "-frames:v", "1", "-c:v", "libwebp", "-quality", "80", poster);
console.log(`${out}: ${end}s`);
