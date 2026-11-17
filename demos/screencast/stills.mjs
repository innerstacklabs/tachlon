// The book's chapter stills (gantt#1558): j2-navigate, keys, j6-build, j7-change (gantt#1549), j4-checks, j8-progress,
// j9-save (gantt#1553) and the printed page j5-print (gantt#1554). One run plays the named scenes of steps-features.mjs in
// order, then drives the page's own controls for the chapters that have no scene (J4, J5, J8, J9), and takes every still
// through the same hideForShot as `ctx.shot` in clip.mjs. Each still is a 1280 px wide `.webp` under 300 KB.
//
//   node demos/screencast/stills.mjs --out <dir> [--only <still>] [--dist <dir>] [--dry]
//
// --dry prints the list below and opens no browser. --only writes one still (the scenes before it still play). A still is
// recorded on the hub into a scratch folder and copied into book/src/media by hand (demos/screencast/README.md, "Where to
// record"): into book/src/media itself the run applies clip.mjs's WebGPU check and writes nothing if it fails.

import { mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { checkHostedDist, DEFAULT_DIST, drawBadges, guardClip, IMAGE_BYTES, writesToBookMedia } from "./clip.mjs";
import { runScenes } from "./ctx.mjs";
import { HOSTED_QUERY } from "./hosted-build.mjs";
import { chromePath, loadChromium, measureFps, preRoll, waitUntilReady } from "./record/browser.mjs";
import { execFile, ffmpegPath, stopChildren } from "./record/media.mjs";
import { hideForShot, installOverlay, showAfterShot } from "./record/overlay.mjs";
import { serveDir } from "./serve.mjs";
import { acceptOpenAsk } from "./steps/helpers.mjs";
import * as features from "./steps-features.mjs";

/** The scenes played in order: drag-bar needs the critical task scroll-to-critical shows, and find comes before it because a drag leaves the Show moved button on the page. */
export const SCENES = ["scroll-to-critical", "find", "park", "insert-delete", "cell-cursor", "drag-bar"];
/** The stills in the order they are taken: the ones a scene's `ctx.shot` writes, then the ones the page's own controls make. */
export const STILLS = [
  { name: "j2-navigate", scene: "find" },
  { name: "j6-build", scene: "insert-delete" },
  { name: "keys", scene: "cell-cursor" },
  { name: "j7-change", scene: "drag-bar" },
  { name: "j4-checks", control: "the Checks button on ?sample=office-fitout" },
  { name: "j9-save", control: "the Open menu on ?sample=office-fitout" },
  { name: "j5-print", control: "Save image (PNG) in the Open menu on ?sample=office-fitout, the printed page scaled to 1280 px wide" },
  { name: "j8-progress", control: "fixtures/fit-out.xer opened on ?sample=demo-50k, five weeks in" },
];

/** The dry list: one line per still, what makes it. */
export const dryList = () => STILLS.map((s) => `${s.name}  ${s.scene ? `scene ${s.scene}, after ${SCENES.slice(0, SCENES.indexOf(s.scene)).join(", ")}` : s.control}`);

/** The pointer the drag leaves on a bar would show a hover card over the table in the insert still: park it on empty chart. */
const park = { name: "park", caption: "park", run: async (c) => { const box = await c.canvasBox(); await c.page.mouse.move(box.x + box.width * 0.85, box.y + c.AXIS + c.ROW * 8); await c.hold(2); } };

async function main() {
  const { values } = parseArgs({ args: process.argv.slice(2), options: { out: { type: "string" }, only: { type: "string" }, dist: { type: "string" }, dry: { type: "boolean" } } });
  if (values.dry) { console.log(dryList().join("\n")); return 0; }
  if (!values.out || (values.only && !STILLS.some((s) => s.name === values.only))) throw Object.assign(new Error(`usage: node stills.mjs --out <dir> [--only <${STILLS.map((s) => s.name).join("|")}>] [--dist <dir>] [--dry]`), { exitCode: 2 });
  const out = resolve(values.out);
  const dist = resolve(values.dist ?? DEFAULT_DIST);
  const want = (...names) => names.some((n) => !values.only || values.only === n);
  await checkHostedDist(dist);
  const temp = await mkdtemp(join(tmpdir(), "gantt-stills-"));
  const taken = [];
  let browser;
  let server;
  const watchdog = setTimeout(() => { console.error("stills timed out after 10 minutes"); process.exit(1); }, 10 * 60_000);
  watchdog.unref();
  try {
    server = await serveDir(dist);
    browser = await (await loadChromium()).launch({ headless: true, executablePath: await chromePath() });
    let page;
    let failure;
    const frames = (n) => page.evaluate((count) => new Promise((done) => { let left = count; const tick = () => (--left <= 0 ? done() : requestAnimationFrame(tick)); requestAnimationFrame(tick); }), n);
    // Each open is a new browser context, so the view an earlier scene left (the zoom, the scroll) cannot reach the next still. A sample is
    // open once its zoom level has held for 30 frames: a still taken while the view is still moving shows another axis (measured on the hub).
    const open = async (query) => {
      await page?.context().close();
      const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, acceptDownloads: true });
      await installOverlay(context);
      page = await context.newPage();
      page.on("pageerror", (err) => { failure ??= err; });
      await page.goto(`http://127.0.0.1:${server.port}/${query}`);
      await waitUntilReady(page);
      await page.waitForFunction(() => { const s = window.gantt.status(); if (window.settled?.level !== s.level) window.settled = { level: s.level, since: s.frames }; return s.frames - window.settled.since >= 30; }, null, { polling: "raf" });
    };
    const keep = (name, png) => taken.push({ name, png });
    const shot = async (name, badges = []) => {
      if (!want(name)) return;
      const png = join(temp, `${name}.png`);
      await hideForShot(page);
      await page.evaluate(drawBadges, badges);
      await page.screenshot({ path: png });
      await page.evaluate(() => { for (const el of document.querySelectorAll(".sc-badge")) el.remove(); });
      await showAfterShot(page);
      keep(name, png);
    };
    const menu = async () => { await page.locator("#open-button").click(); await page.locator("#sample-list").waitFor({ state: "visible" }); await frames(10); };
    const close = async () => { await page.keyboard.press("Escape"); await frames(5); };

    await open(HOSTED_QUERY);
    const fps = await measureFps(page);
    if (want(...STILLS.filter((s) => s.scene).map((s) => s.name))) {
      const { ctx } = await preRoll(page, fps, features.setup);
      ctx.shot = shot;
      await runScenes(ctx, SCENES.map((name) => (name === "park" ? park : features.steps.find((s) => s.name === name))));
    }
    if (want("j4-checks", "j9-save", "j5-print")) {
      await open("?sample=office-fitout");
      if (want("j4-checks")) { await page.locator("#checks-button").click(); await page.locator("#checks-dialog").waitFor({ state: "visible" }); await frames(10); await shot("j4-checks"); await close(); }
      if (want("j9-save")) { await menu(); await shot("j9-save"); await close(); }
      if (want("j5-print")) {
        await menu();
        const download = page.waitForEvent("download");
        await page.locator("[data-export-image]").click();
        const png = join(temp, "j5-print.png");
        await (await download).saveAs(png);
        keep("j5-print", png);
      }
    }
    if (want("j8-progress")) {
      await open(HOSTED_QUERY);
      acceptOpenAsk(page);
      await page.locator("#open-input").setInputFiles(new URL("./fixtures/fit-out.xer", import.meta.url).pathname);
      await page.waitForFunction(() => window.gantt.opened?.name === "fit-out.xer" && window.gantt.status().n === window.gantt.opened.tasks, null, { timeout: 60_000 });
      await page.waitForFunction(() => (document.getElementById("data-date")?.textContent ?? "").includes("2026-10-26") && document.querySelector("#table-head .percent") !== null, null, { timeout: 60_000 });
      await frames(20);
      await shot("j8-progress");
    }
    if (failure) throw new Error(`page broke: ${failure.message}`);
    // The backend is read after the last scene, as clip.mjs does; into book/src/media nothing is written unless it passes.
    guardClip(out, writesToBookMedia(out) ? { backend: (await page.evaluate(() => window.gantt.status())).backend, fps } : undefined, "stills");
    await mkdir(out, { recursive: true });
    const problems = [];
    for (const { name, png } of taken) {
      const webp = join(out, `${name}.webp`);
      await execFile(ffmpegPath(), ["-y", "-i", png, "-vf", "scale=1280:-1:flags=lanczos", "-c:v", "libwebp", "-quality", "85", webp]);
      const bytes = (await stat(webp)).size;
      if (bytes > IMAGE_BYTES) problems.push(`${name}.webp is ${bytes} bytes; the budget is under ${IMAGE_BYTES}`);
      console.log(`still ${name}: ${bytes} bytes in ${out}`);
    }
    for (const problem of problems) console.error(problem);
    return problems.length > 0 ? 3 : 0;
  } finally {
    clearTimeout(watchdog);
    await stopChildren();
    await browser?.close().catch(() => {});
    await server?.close().catch(() => {});
    await rm(temp, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  try {
    process.exitCode = await main();
  } catch (err) {
    console.error(err?.message ?? err);
    process.exitCode = err?.exitCode ?? 1;
  }
}
