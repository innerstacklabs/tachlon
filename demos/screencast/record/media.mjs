import { existsSync } from "node:fs";
import { copyFile, stat } from "node:fs/promises";
import { execFile as execFileCallback } from "node:child_process";
import { basename, join } from "node:path";

// Width first while 12 fps fits, then fewer frames at 560 px before the width drops again:
// the 16-step journey (gs#314 look, insert/delete) is 8 MB at 560 px and 12 fps.
const gifSettings = [
  [800, 12], [720, 12], [640, 12], [560, 12],
  [560, 10], [560, 8], [560, 6], [480, 8], [480, 6],
];
const gifLimit = 5_000_000;

// One feature clip per scene (gs#152): a GIF under 600,000 bytes, at most 640 px wide, 8 fps and 12 s. The widest rung that fits wins.
export const clipLimit = 600_000;
export const clipMaxSeconds = 12;
const clipSettings = [[640, 8], [560, 8], [480, 8], [480, 6], [400, 6], [320, 6]];

const children = new Set();
export function execFile(file, args) {
  return new Promise((done, fail) => {
    const child = execFileCallback(file, args, (err, stdout, stderr) => {
      children.delete(child);
      if (err) {
        err.stdout = stdout;
        err.stderr = stderr;
        fail(err);
      } else {
        done({ stdout, stderr });
      }
    });
    children.add(child);
  });
}

export async function stopChildren() {
  await Promise.all([...children].map((child) => new Promise((done) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      done();
      return;
    }
    child.once("close", done);
    child.kill("SIGKILL");
  })));
}


export function ffmpegPath() {
  if (process.env.FFMPEG) return process.env.FFMPEG;
  if (existsSync("/opt/homebrew/bin/ffmpeg")) return "/opt/homebrew/bin/ffmpeg";
  for (const dir of (process.env.PATH ?? "").split(":")) if (dir && existsSync(join(dir, "ffmpeg"))) return join(dir, "ffmpeg");
  throw new Error("no ffmpeg found; set FFMPEG");
}

export async function makeMedia(source, out, name, trim, ffmpeg, temp) {
  const mp4 = join(out, `${name}.mp4`);
  await execFile(ffmpeg, ["-y", "-i", source, "-ss", String(trim), "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "23", "-pix_fmt", "yuv420p", "-vf", "scale=1280:720:flags=lanczos", "-r", "30", "-movflags", "+faststart", mp4]);
  const mp4Bytes = (await stat(mp4)).size;
  const tried = [];
  let gif;
  for (const [width, fps] of gifSettings) {
    const attempt = join(temp, `gif-${width}-${fps}.gif`);
    const filter = `fps=${fps},scale=${width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=64:stats_mode=diff[p];[s1][p]paletteuse=dither=none:diff_mode=rectangle`;
    await execFile(ffmpeg, ["-y", "-i", source, "-ss", String(trim), "-filter_complex", filter, "-loop", "0", attempt]);
    const bytes = (await stat(attempt)).size;
    tried.push({ width, fps, bytes });
    console.log(`gif ${width}px ${fps} fps: ${bytes} bytes`);
    if (bytes < gifLimit) {
      await copyFile(attempt, join(out, `${name}.gif`));
      gif = { bytes, width, fps };
      break;
    }
  }
  return { mp4: { bytes: mp4Bytes }, gif: gif ?? tried.at(-1), tried, accepted: Boolean(gif) };
}

/**
 * Cut [start, end] seconds of the recorded video to `dest` as a GIF, trying each rung of the ladder until one is under
 * `limit` bytes. Writes `dest` only for a clip that fits; a window over 12 s, or a scene that fits no rung, throws and leaves
 * nothing at `dest`.
 */
export async function makeClip(source, dest, start, end, ffmpeg, temp, { limit = clipLimit } = {}) {
  const seconds = end - start;
  if (!Number.isFinite(seconds) || seconds <= 0) throw new Error(`clip window ${start}..${end} s is empty`);
  if (seconds > clipMaxSeconds) throw new Error(`clip window is ${seconds.toFixed(1)} s; the limit is ${clipMaxSeconds} s`);
  const tried = [];
  for (const [width, fps] of clipSettings) {
    const attempt = join(temp, `clip-${basename(dest)}-${width}-${fps}.gif`);
    const filter = `fps=${fps},scale=${width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=64:stats_mode=diff[p];[s1][p]paletteuse=dither=none:diff_mode=rectangle`;
    await execFile(ffmpeg, ["-y", "-i", source, "-ss", String(start), "-to", String(end), "-filter_complex", filter, "-loop", "0", attempt]);
    const bytes = (await stat(attempt)).size;
    tried.push({ width, fps, bytes });
    if (bytes < limit) {
      await copyFile(attempt, dest);
      return { bytes, width, fps, seconds: Number(seconds.toFixed(2)), tried };
    }
  }
  throw new Error(`no rung fits ${limit} bytes; tried ${tried.map((item) => `${item.width}px/${item.fps}fps=${item.bytes}`).join(", ")}`);
}

export async function mp4Duration(ffmpeg, file, fallback) {
  let stderr = "";
  try {
    ({ stderr } = await execFile(ffmpeg, ["-i", file]));
  } catch (err) {
    stderr = err.stderr ?? "";
  }
  const match = /Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)/.exec(stderr);
  return match ? Number(match[1]) * 3600 + Number(match[2]) * 60 + Number(match[3]) : fallback;
}

