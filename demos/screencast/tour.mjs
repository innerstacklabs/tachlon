// The annotated still of the book's "Open a schedule" chapter (gantt#512): the office fit-out sample open in the hosted build,
// four badges from clip.mjs's drawBadges, a .webp. A still with no frame rate, so the hub's software GL is fine.
//   node demos/screencast/tour.mjs <out.webp> [<hosted build dir, default apps/hosted/dist>]
import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DEFAULT_DIST, IMAGE_BYTES, checkHostedDist, drawBadges } from "./clip.mjs";
import { serveDir } from "./serve.mjs";
import { chromePath, loadChromium, waitUntilReady } from "./record/browser.mjs";
import { execFile, ffmpegPath } from "./record/media.mjs";

const BADGES = [ // pixel positions on the 1280x720 page
  { n: 1, point: { x: 200, y: 59 } }, // the task table's header
  { n: 2, point: { x: 1100, y: 120 } }, // the timeline
  { n: 3, point: { x: 800, y: 18 } }, // the toolbar
  { n: 4, point: { x: 960, y: 288 } }, // a critical bar: "Prepare permit application"
];
const [out, dist = DEFAULT_DIST] = process.argv.slice(2);
if (!out) throw new Error("usage: node tour.mjs <out.webp> [<hosted build dir>]");
await checkHostedDist(dist);
const temp = await mkdtemp(join(tmpdir(), "gantt-tour-"));
const server = await serveDir(resolve(dist));
const browser = await (await loadChromium()).launch({ headless: true, executablePath: await chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await page.goto(`http://127.0.0.1:${server.port}/?sample=office-fitout`);
  await waitUntilReady(page);
  await page.waitForFunction(() => window.gantt.status().n === 59 && document.querySelector("#table-rows .row") !== null);
  await page.locator("#first-screen-line button").click(); // the sample notice, dismissed as a reader would
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await page.evaluate(drawBadges, BADGES);
  await page.screenshot({ path: join(temp, "tour.png") });
  await execFile(ffmpegPath(), ["-y", "-i", join(temp, "tour.png"), "-c:v", "libwebp", "-quality", "85", resolve(out)]);
  const bytes = (await stat(resolve(out))).size;
  console.log(`tour: ${bytes} bytes in ${resolve(out)}`);
  if (bytes > IMAGE_BYTES) throw new Error(`${out} is ${bytes} bytes; the budget is under ${IMAGE_BYTES}`);
} finally {
  await Promise.all([browser.close(), server.close(), rm(temp, { recursive: true, force: true })]);
}
