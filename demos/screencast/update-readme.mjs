import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { sourceLabel } from "./label.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const readmePath = resolve(root, "README.md");
const metadataPath = resolve(root, "docs/media/screencast.json");
const packagePath = resolve(root, "packages/gantt/package.json"); // the product's name is written there, not here

function usage() {
  throw Object.assign(new Error("usage: node demos/screencast/update-readme.mjs [--check | --upload]"), { exitCode: 2 });
}

function requireField(value, path) {
  if (value === undefined || value === null) {
    throw new Error(`docs/media/screencast.json is missing ${path}`);
  }
  return value;
}

// A bare GitHub attachment URL on its own line renders as a video player. The upload is a manual
// step (gs issue: screencast upload), so a re-record carries the URL already in the README block
// through, and run.sh warns that it still points at the previous recording.
const ASSET_LINE = /^https:\/\/github\.com\/user-attachments\/assets\/[0-9a-f-]+$/m;

function existingAssetUrl(readme) {
  const marked = /^<!-- screencast:begin[^\r\n]*\r?\n[\s\S]*?^<!-- screencast:end -->$/m.exec(readme);
  const found = marked ? ASSET_LINE.exec(marked[0]) : null;
  return found ? found[0] : null;
}

function makeBlock(metadata, assetUrl, name) {
  const machine = requireField(metadata.machine, "machine");
  const cpu = requireField(metadata.cpu, "cpu");
  const date = requireField(metadata.date, "date");
  const commit = requireField(metadata.commit, "commit");
  const source = sourceLabel({ branch: metadata.branch, commit });
  const backend = requireField(metadata.backend, "backend");
  requireField(metadata.gif, "gif");
  requireField(metadata.mp4, "mp4");
  const gifBytes = requireField(metadata.gif.bytes, "gif.bytes");
  const mp4Bytes = requireField(metadata.mp4.bytes, "mp4.bytes");
  if (!Number.isFinite(gifBytes) || !Number.isFinite(mp4Bytes)) {
    throw new Error("docs/media/screencast.json fields gif.bytes and mp4.bytes must be numbers");
  }
  const mp4Mb = (mp4Bytes / 1e6).toFixed(1);
  // One video, never two: with an uploaded attachment GitHub shows its own player, so the GIF is
  // left out; without one the GIF plays inline, links to the product's site, and a comment says the
  // uploaded MP4 (${mp4Mb} MB) replaces it.
  const site = `https://${name}.com`;
  const video = assetUrl
    ? assetUrl
    : `<a href="${site}"><img src="docs/media/screencast.gif" alt="The chart on a 50,000-task schedule, a scripted walk through what it can do: each step has its caption in the video" width="640"></a>`;
  const overview = `Watch the narrated overview at [${name}.com](${site}).`;
  const standIn = assetUrl ? "" : `<!-- screencast: the GIF stands in until docs/media/screencast.mp4 (${mp4Mb} MB) is uploaded as a GitHub attachment; its URL on a line of its own then replaces the image -->\n`;
  // The recording metadata is an HTML comment: the README is for a first-time reader (gs#448), and
  // the contributor rule for the screencast is in docs/CONTRIBUTING.md.
  return `<!-- screencast:begin (written by demos/screencast/run.sh; do not edit by hand) -->

${video}

${overview}

${standIn}<!-- screencast: recorded headless on a ${machine} (${cpu}, ${backend}) on ${date} ${source} -->
<!-- screencast:end -->`;
}

function updatedReadme(readme, block) {
  const marked = /^<!-- screencast:begin[^\r\n]*\r?\n[\s\S]*?^<!-- screencast:end -->$/m;
  if (marked.test(readme)) return readme.replace(marked, block);

  const lines = readme.split(/\r?\n/);
  const heading = lines.findIndex((line) => line.startsWith("# "));
  if (heading === -1) throw new Error("README.md has no top-level heading");
  let rest = heading + 1;
  while (rest < lines.length && lines[rest].trim() === "") rest++;
  lines.splice(heading + 1, rest - heading - 1, "", ...block.split("\n"), "");
  return lines.join("\n");
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length > 1 || (args.length === 1 && args[0] !== "--check" && args[0] !== "--upload")) usage();
  const check = args[0] === "--check";
  const upload = args[0] === "--upload"; // run.sh --upload replaces the carried-over URL right after this, so no warning
  const metadata = JSON.parse(await readFile(metadataPath, "utf8"));
  const readme = await readFile(readmePath, "utf8");
  const assetUrl = existingAssetUrl(readme);
  const name = JSON.parse(await readFile(packagePath, "utf8")).name;
  const next = updatedReadme(readme, makeBlock(metadata, assetUrl, name));
  if (assetUrl && !check && !upload && next !== readme) {
    console.warn("update-readme.mjs: the README video URL is carried over from the previous recording; upload the new MP4 and replace it");
  }
  if (check) {
    if (next !== readme) process.exitCode = 1;
    return;
  }
  await writeFile(readmePath, next);
  console.log("README.md screencast block updated");
}

try {
  await main();
} catch (error) {
  console.error(`update-readme.mjs: ${error?.message ?? error}`);
  process.exitCode = error?.exitCode ?? 1;
}
