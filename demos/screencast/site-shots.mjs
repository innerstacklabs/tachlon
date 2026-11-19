// The product page's pictures of the chart (to#107): the nine `.webp` files in apps/site/shots/, taken from the hosted build with the
// page's own chart colors (the two `.gantt[data-theme]` rules in apps/site/home.css), at the sizes and crops the page's <img> tags name.
//   node demos/screencast/site-shots.mjs [--out <dir>] [--only <name>] [--dist <dir>] [--dry]
// --out defaults to apps/site/shots. The files are still pictures, so the hub's software GL is fine (README.md, "Where to record").
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { checkHostedDist, DEFAULT_DIST } from "./clip.mjs";
import { chromePath, loadChromium, waitUntilReady } from "./record/browser.mjs";
import { execFile, ffmpegPath, stopChildren } from "./record/media.mjs";
import { serveDir } from "./serve.mjs";
import { acceptOpenAsk } from "./steps/helpers.mjs";
const OUT = fileURLToPath(new URL("../../apps/site/shots", import.meta.url));
const FIT_OUT = fileURLToPath(new URL("./fixtures/fit-out.xer", import.meta.url));
const part = (name) => `[data-part=${name}]`;
/** One group is one page state; each picture of it is a clip `[x, y, width, height]` in CSS px (the files are 2x). `moved` drags a bar first. */
export const GROUPS = [
  { size: [1440, 860], query: "?sample=office-fitout", progress: true, shots: [{ name: "hero-fitout" }] },
  { size: [1180, 700], query: "?sample=office-fitout", progress: true, critical: true, shots: [{ name: "critical" }] },
  { size: [1180, 700], query: "?sample=demo-50k", shots: [{ name: "speed-50k" }] },
  { size: [1440, 860], query: "?sample=medium-5k", progress: true, shots: [{ name: "xer-5k" }] },
  { size: [1440, 900], file: FIT_OUT, progress: true, shots: [
    { name: "hero-light" }, { name: "hero-phone", clip: [700, 0, 740, 900] }, { name: "p6", clip: [0, 0, 730, 530] },
    { name: "themes", halves: [[220, 0, 500, 650], [720, 0, 500, 650]] }, { name: "moved", clip: [720, 0, 720, 590], drag: "Fire sprinkler drops" },
  ] },
];
/** The page's chart tokens per theme, from its own stylesheet: `{ light: [[name, value], ...], dark: [...] }`. */
async function tokens() {
  const css = await readFile(new URL("../../apps/site/home.css", import.meta.url), "utf8");
  const found = {};
  for (const [, theme, body] of css.matchAll(/^\.demo-stage \.gantt\[data-theme="(light|dark)"\] \{([^}]*)\}/gm)) found[theme] = body.split(";").map((d) => d.split(/:(.*)/s).map((s) => s.trim())).filter(([name]) => name);
  if (!found.light || !found.dark) throw new Error("apps/site/home.css has no .demo-stage .gantt[data-theme] rule for light and dark");
  return found;
}
async function openPage(browser, server, group, theme, colors) {
  const [width, height] = group.size;
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 2, acceptDownloads: true });
  const page = await context.newPage();
  const failures = [];
  page.on("pageerror", (err) => failures.push(err));
  await page.goto(`http://127.0.0.1:${server.port}/${group.query ?? "?sample=demo-50k"}&theme=${theme}`);
  await waitUntilReady(page);
  // The chart re-reads its tokens when the root's `style` changes, so inline tokens on the root take the page's colors without a stylesheet.
  await page.evaluate((list) => { for (const [name, value] of list) document.getElementById("app").style.setProperty(name, value); }, colors[theme]);
  if (group.file) {
    acceptOpenAsk(page);
    await page.locator("#open-input").setInputFiles(group.file);
    await page.waitForFunction(() => window.gantt.opened?.name === "fit-out.xer" && window.gantt.status().n === window.gantt.opened.tasks, null, { timeout: 60_000 });
  }
  // Settled: the zoom level has held for 30 frames and the open animation (360 ms a bar) is over. The Progress button is read after that: a file's opening turns it on by itself.
  const settle = () => page.waitForFunction(() => { const s = window.gantt.status(); if (window.settled?.level !== s.level) window.settled = { level: s.level, since: s.frames }; return s.frames - window.settled.since >= 30; }, null, { polling: "raf" });
  await settle();
  const note = page.locator("#first-screen-line button");
  if (await note.isVisible()) await note.click();
  const progress = page.locator(part("progress-toggle"));
  if (group.progress && (await progress.getAttribute("aria-pressed")) !== "true") await progress.click();
  if (group.critical) { await page.locator(part("filters-button")).click(); await page.locator(part("critical-toggle")).click(); await page.keyboard.press("Escape"); }
  await settle();
  await page.mouse.move(0, 0);
  await page.evaluate(() => new Promise((done) => { let n = 20; const tick = () => (--n <= 0 ? done() : requestAnimationFrame(tick)); requestAnimationFrame(tick); }));
  if (failures.length > 0) throw new Error(`page broke: ${failures[0].message}`);
  return page;
}

/** Drag the named task's bar a week right and leave the pointer on it: the dashed old place, the Show moved bubble and the link dot are what the picture shows. */
async function dragBar(page, name) {
  const row = await page.locator("#table-body", { hasText: name }).locator(`text="${name}"`).first().boundingBox();
  const y = row.y + row.height / 2;
  for (let x = 720; x < 1440; x += 4) {
    await page.mouse.move(x, y);
    if (await page.evaluate(() => window.gantt.status().hover?.task != null)) {
      await page.mouse.down();
      await page.mouse.move(x + 20, y, { steps: 4 });
      await page.mouse.move(x + 76, y, { steps: 8 });
      await page.mouse.up();
      await page.addStyleTag({ content: "[data-part=hover-card] { visibility: hidden !important; }" });
      await page.waitForFunction(() => window.gantt.status().moved !== null && !window.gantt.status().drag?.active, null, { polling: "raf" });
      return;
    }
  }
  throw new Error(`no bar found at the row of ${name}`);
}

async function main() {
  const { values } = parseArgs({ args: process.argv.slice(2), options: { out: { type: "string" }, only: { type: "string" }, dist: { type: "string" }, dry: { type: "boolean" } } });
  const names = GROUPS.flatMap((g) => g.shots.map((s) => s.name));
  if (values.dry) { console.log(names.join("\n")); return 0; }
  if (values.only && !names.includes(values.only)) throw Object.assign(new Error(`usage: node site-shots.mjs [--out <dir>] [--only <${names.join("|")}>] [--dist <dir>] [--dry]`), { exitCode: 2 });
  const out = resolve(values.out ?? OUT);
  const dist = resolve(values.dist ?? DEFAULT_DIST);
  await checkHostedDist(dist);
  const temp = await mkdtemp(join(tmpdir(), "gantt-site-shots-"));
  const colors = await tokens();
  let server;
  let browser;
  try {
    server = await serveDir(dist);
    browser = await (await loadChromium()).launch({ headless: true, executablePath: await chromePath() });
    for (const group of GROUPS) {
      const todo = group.shots.filter((s) => !values.only || values.only === s.name);
      if (todo.length === 0) continue;
      const [width, height] = group.size;
      const page = await openPage(browser, server, group, "light", colors);
      for (const shot of todo) {
        if (shot.drag) await dragBar(page, shot.drag);
        const halves = shot.halves ?? [shot.clip ?? [0, 0, width, height]];
        const dark = shot.halves ? await openPage(browser, server, group, "dark", colors) : null;
        const pngs = [];
        for (const [i, [x, y, w, h]] of halves.entries()) {
          pngs.push(join(temp, `${shot.name}-${i}.png`));
          await (i === 0 ? page : dark).screenshot({ path: pngs[i], clip: { x, y, width: w, height: h } });
        }
        const webp = join(out, `${shot.name}.webp`);
        const inputs = pngs.flatMap((p) => ["-i", p]);
        await execFile(ffmpegPath(), ["-y", ...inputs, ...(pngs.length > 1 ? ["-filter_complex", "hstack=inputs=2"] : []), "-c:v", "libwebp", "-quality", "80", webp]);
        console.log(`${shot.name}: ${(await stat(webp)).size} bytes`);
      }
    }
    return 0;
  } finally {
    await stopChildren();
    await browser?.close().catch(() => {});
    await server?.close().catch(() => {});
    await rm(temp, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main();
  } catch (err) {
    console.error(err?.message ?? err);
    process.exitCode = err?.exitCode ?? 1;
  }
}
