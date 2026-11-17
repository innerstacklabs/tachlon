// Records each segment of the setup screencast with Playwright video and logs when each step starts.
// Usage: node record.mjs <work dir>   (env APP_PORT: the Vite app, which run.sh has already started)
// Reads <work>/www (the terminal and code views, served on a free port); writes <work>/clips/raw/<segment>.webm and marks.json.
import { mkdirSync, writeFileSync, renameSync } from "node:fs";
import { join } from "node:path";
import { serveDir } from "../screencast/serve.mjs";
import { chromePath, loadChromium, useSystemFfmpegForVideo } from "../screencast/record/browser.mjs";

const work = process.argv[2];
const pkg = process.env.PKG;
const APP = `http://localhost:${process.env.APP_PORT}/`;
const raw = join(work, "clips", "raw");
mkdirSync(raw, { recursive: true });
await useSystemFfmpegForVideo(work);
const site = await serveDir(join(work, "www"));
const W = `http://127.0.0.1:${site.port}`;
const browser = await (await loadChromium()).launch({
  executablePath: await chromePath(),
  args: ["--enable-unsafe-webgpu", "--use-angle=swiftshader", "--enable-webgl", "--ignore-gpu-blocklist", "--hide-scrollbars"],
});
const marks = {};
const hold = (page, ms) => page.waitForTimeout(ms);
// The chart reports the backend it settled on in the console.
const waitChart = (page) => page.waitForEvent("console", { predicate: (m) => /gantt: (backend WebGL2|running on WebGL2)/.test(m.text()), timeout: 8000 }).catch(() => null);

async function segment(name, fn) {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: join(work, "clips", "tmp"), size: { width: 1280, height: 720 } } });
  await ctx.addInitScript(() => {
    document.addEventListener("DOMContentLoaded", () => {
      const s = document.createElement("style");
      s.textContent = "*{cursor:none !important}";
      document.head.appendChild(s);
      // A near-invisible 1px ticker so the recorder keeps emitting frames while the page is still.
      const t = document.createElement("div");
      t.style.cssText = "position:fixed;right:0;bottom:0;width:1px;height:1px;z-index:2147483647;pointer-events:none";
      document.body.appendChild(t);
      let on = false;
      setInterval(() => { on = !on; t.style.background = on ? "rgba(128,128,128,0.06)" : "rgba(128,128,128,0.03)"; }, 50);
    });
  });
  const page = await ctx.newPage();
  const t0 = Date.now();
  const rec = { steps: [] };
  const mark = (label) => { const t = (Date.now() - t0) / 1000; rec.steps.push({ label, t }); console.log(name, t.toFixed(2), label); return t; };
  page.on("pageerror", (e) => console.log(name, "PAGEERROR", e.message));
  await fn(page, mark, rec);
  rec.end = mark("end");
  const video = page.video();
  await ctx.close();
  renameSync(await video.path(), join(raw, `${name}.webm`));
  marks[name] = rec;
}

async function term(page, mark, rec, scene, tail) {
  await page.goto(`${W}/term.html?s=${scene}`);
  await page.waitForLoadState("load");
  await hold(page, 200);
  rec.start = mark("terminal shown");
  await page.waitForFunction(() => window.done === true, null, { timeout: 120000 });
  await hold(page, tail);
}
async function code(page, mark, rec, query, ms, label) {
  await page.goto(`${W}/code.html?${query}`);
  await page.waitForFunction(() => window.ready === true);
  await hold(page, 300);
  rec.start = mark(label);
  await hold(page, ms);
}
const scrollToInput = (page, behavior) => page.evaluate((behavior) => {
  const i = document.querySelector("input[type=file]");
  window.scrollTo({ top: i.getBoundingClientRect().top + window.scrollY - 12, behavior });
}, behavior);

await segment("1a-term", (p, m, r) => term(p, m, r, 1, 1500));
await segment("2a-chart", (p, m, r) => code(p, m, r, `f=${pkg}-chart.tsx&name=src/${pkg}-chart.tsx&hl=21,30`, 11000, `${pkg}-chart.tsx`));
await segment("2b-app", (p, m, r) => code(p, m, r, "f=App.stage1.tsx&name=src/App.tsx&from=1&to=22&hl=6,14", 9000, "App.tsx"));
await segment("3a-dev", (p, m, r) => term(p, m, r, 3, 800));
await segment("3b-sample", async (page, mark, rec) => {
  const ready = waitChart(page);
  await page.goto(APP);
  await ready;
  await hold(page, 800);
  rec.start = mark("sample chart shown");
  await hold(page, 3500);
  const box = await page.locator(".gantt").first().boundingBox();
  await page.mouse.move(box.x + box.width * 0.62, box.y + 110);
  mark("zoom in (ctrl+wheel)");
  await page.keyboard.down("Control");
  for (let i = 0; i < 5; i++) { await page.mouse.wheel(0, -100); await hold(page, 260); }
  await hold(page, 1600);
  mark("zoom out (ctrl+wheel)");
  for (let i = 0; i < 5; i++) { await page.mouse.wheel(0, 100); await hold(page, 260); }
  await page.keyboard.up("Control");
  await hold(page, 2200);
});
await segment("4a-open", (p, m, r) => code(p, m, r, "f=open-schedule.tsx&name=src/open-schedule.tsx&hl=10,11", 8000, "open-schedule.tsx"));
await segment("4b-xer", async (page, mark, rec) => {
  const ready = waitChart(page);
  await page.goto(APP);
  await ready;
  await scrollToInput(page, "auto");
  await hold(page, 800);
  rec.start = mark("file input shown");
  await hold(page, 1500);
  const loaded = page.waitForEvent("console", { predicate: (m) => /tasks, ready/.test(m.text()) && !/ 3 tasks/.test(m.text()), timeout: 10000 }).catch(() => null);
  mark("choose fit-out.xer");
  await page.setInputFiles("input[type=file]", join(import.meta.dirname, "../screencast/fixtures/fit-out.xer"));
  await loaded;
  await hold(page, 700);
  await scrollToInput(page, "smooth");
  mark("xer drawn");
  await hold(page, 4500);
  const box = await page.locator(".gantt").nth(1).boundingBox();
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.6);
  mark("scroll rows");
  for (let i = 0; i < 6; i++) { await page.mouse.wheel(0, 90); await hold(page, 200); }
  await hold(page, 3000);
});
await segment("5a-cdnsrc", (p, m, r) => code(p, m, r, "f=cdn-index.html&name=cdn/index.html&from=1&to=23&hl=6,7,11,13", 8000, "cdn/index.html"));
await segment("5b-cdn", async (page, mark, rec) => {
  const ready = waitChart(page);
  await page.goto(`${W}/cdn/index.html`);
  await ready;
  await hold(page, 800);
  rec.start = mark("cdn page drawn");
  await hold(page, 8000);
});
await segment("6a-doctor", (p, m, r) => term(p, m, r, 6, 3000));

await browser.close();
await site.close();
writeFileSync(join(work, "clips", "marks.json"), JSON.stringify(marks, null, 1));
console.log("recorded");
