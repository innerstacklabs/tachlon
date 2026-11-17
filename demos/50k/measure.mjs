// Runs the scripted measurement (the page's `?measure=1`) in a browser and
// writes its JSON result. Started by `demos/50k/run.sh --measure`.
//
//   node demos/50k/measure.mjs [options]
//
//   --out <dir>        where the JSON goes (default demos/50k/results/)
//   --machine <label>  the machine, as written in the result (default: host name and CPU)
//   --headless         no window, on the software renderer: a smoke run, not a result
//   --chrome <path>    a Chrome or Chromium binary (or CHROME=<path>)
//   --channel <name>   a Playwright channel such as chrome or msedge (default chrome)
//   --viewport WxH     a fixed window size (default: the browser's own window, native pixel ratio)
//   --url <url>        use a server that is already running (default: start `pnpm preview`)
//   --param k=v        a page option, repeatable: n, repeats, warmup, dragMs, panMs, dragTarget (critical), editRepeats, baseline, refreshHz, backend
//   --chrome-arg <f>   a Chromium flag, repeatable (e.g. --enable-unsafe-webgpu, --disable-gpu-vsync);
//                      recorded in the result as `chromeArgs`
//   --open <file>      after the scripted run, open this P6 XER or MS Project XML file through the page's file
//                      input, in the same page, once as a warm-up and then --open-repeats times (the schedule is
//                      replaced in place each time), and add `open` to the result (docs/MEASURING.md, "Open time")
//   --open-repeats <n> recorded opens (default 5, at least 1)
//   --open-project <p> for a file with several projects: the proj_id to choose, or all (default all)
//   --schedule <file>  run the frame phases on this P6 XER or MS Project XML file instead of the stress
//                      fixture (the page's ?measure=1&schedule=1); the result records fixture "file" and the
//                      file's SHA-256 (docs/MEASURING.md, "Measuring a file")
//
// Do not touch the window while it runs: the script drives the page itself.
// Ctrl-C (SIGINT) or SIGTERM stops the preview and exits 130 or 143; a second signal exits at once.
import { spawn } from "node:child_process";
import { accessSync, constants, mkdirSync, readFileSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { cpus, hostname, platform } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { OPEN_PHASES, measureOpens, openArguments, openVerdict } from "./measure/open.mjs";
import { freePort, parseParam, playwrightChromium, stopGroup } from "./measure/process.mjs";

// The tests and other callers import these from here.
export { OPEN_PHASES, OPEN_SPAN, OPEN_SPAN_TOLERANCE_MS, OPEN_SUM_TOLERANCE_MS, OPEN_TARGET_MS, measureOpens, openArguments, openBlock, openOnce, openVerdict, percentile, summarize } from "./measure/open.mjs";
export { newestChromium, parseParam, stopGroup } from "./measure/process.mjs";

const timeoutMs = 30 * 60_000;
let previewChild;
let previewUrl;
let stopping;

async function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const { chromium } = createRequire(join(root, "packages", "gantt", "package.json"))("@playwright/test");
  const args = process.argv.slice(2);
  const flag = (name) => args.includes(name);
  const option = (name) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const params = new Map();
  const chromeArgs = [];
  args.forEach((a, i) => {
    if (a === "--param") {
      const [k, v] = parseParam(args[i + 1]);
      params.set(k, v);
    }
    if (a === "--chrome-arg") {
      if (!args[i + 1]?.startsWith("--")) throw new Error(`--chrome-arg needs a Chromium flag such as --enable-unsafe-webgpu, got ${args[i + 1]}`);
      chromeArgs.push(args[i + 1]);
    }
  });

  const { file: openFile, repeats: openRepeats, project: openProject } = openArguments(args);
  if (openFile !== undefined) {
    // Fail before a browser or a server starts, not after the minutes of the scripted run.
    try {
      accessSync(openFile, constants.R_OK);
      if (!statSync(openFile).isFile()) throw new Error("not a regular file");
    } catch (err) {
      throw new Error(`--open needs a readable file, got ${openFile}: ${err.message}`);
    }
  }

  const scheduleFile = option("--schedule");
  if (flag("--schedule") && scheduleFile === undefined) throw new Error("--schedule needs a file");
  let scheduleBytes;
  if (scheduleFile !== undefined) {
    try {
      scheduleBytes = readFileSync(scheduleFile);
    } catch (err) {
      throw new Error(`--schedule needs a readable file, got ${scheduleFile}: ${err.message}`);
    }
  }

  const headless = flag("--headless");
  const out = resolve(option("--out") ?? join(root, "demos", "50k", "results"));
  const machine = option("--machine") ?? `${hostname()} (${cpus()[0]?.model ?? "unknown CPU"}, ${platform()})`;
  let browser;
  const handleSignal = (status) => () => {
    if (stopping) {
      try {
        if (previewChild) process.kill(-previewChild.pid, "SIGKILL");
      } catch (err) {
        if (err?.code !== "ESRCH") throw err;
      }
      process.exit(status);
    }
    stopping = (async () => {
      if (browser) {
        await Promise.race([
          Promise.resolve().then(() => browser.close()).catch(() => {}),
          new Promise((done) => setTimeout(done, 10_000)),
        ]);
      }
      // Test-only delay for exercising the race with main's cleanup.
      const signalDelayMs = Number(process.env.GS_MEASURE_SIGNAL_DELAY_MS);
      if (signalDelayMs > 0) await new Promise((done) => setTimeout(done, signalDelayMs));
      if (previewChild && previewUrl) {
        try {
          await stopGroup(previewChild, previewUrl);
        } catch (err) {
          console.error(err);
        }
      }
      process.exit(status);
    })();
    return stopping;
  };
  process.on("SIGINT", handleSignal(130));
  process.on("SIGTERM", handleSignal(143));

  async function serve() {
    const given = option("--url");
    if (given) return { url: given, async stop() {} };
    const port = await freePort();
    const child = spawn("pnpm", ["preview", "--host", "127.0.0.1", "--port", String(port), "--strictPort"], {
      cwd: join(root, "packages", "gantt"),
      stdio: "ignore",
      detached: true,
    });
    previewChild = child;
    const url = `http://127.0.0.1:${port}/`;
    previewUrl = url;
    for (let i = 0; i < 60; i++) {
      try {
        if ((await fetch(url)).ok) return { url, stop: () => stopGroup(child, url) };
      } catch {
        // not up yet
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    await stopGroup(child, url);
    throw new Error("pnpm preview did not start; build first with scripts/build-web.sh");
  }

  const server = await serve();
  let result;
  try {
    // Finding and launching the browser is inside the cleanup: a missing Chrome must not leave the preview server running.
    const launch = { headless, handleSIGINT: false, handleSIGTERM: false, handleSIGHUP: false };
    const chrome = option("--chrome") ?? process.env.CHROME;
    if (chrome) launch.executablePath = chrome;
    else if (headless) launch.executablePath = playwrightChromium();
    else launch.channel = option("--channel") ?? "chrome";
    if (headless) launch.args = ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"];
    if (chromeArgs.length) launch.args = [...(launch.args ?? []), ...chromeArgs];
    browser = await chromium.launch(launch);
    const size = option("--viewport")?.split("x").map(Number) ?? (headless ? [1280, 800] : null);
    const context = await browser.newContext(size ? { viewport: { width: size[0], height: size[1] } } : { viewport: null });
    const page = await context.newPage();
    const query = new URLSearchParams({ measure: "1", machine });
    for (const [k, v] of params) query.set(k, v);
    if (scheduleBytes !== undefined) {
      // The page fetches schedule.dat; the file is served from here, so the built `dist/` is not touched.
      query.set("schedule", "1");
      await page.route("**/schedule.dat", (route) => route.fulfill({ status: 200, contentType: "application/octet-stream", body: scheduleBytes }));
    }
    await page.goto(`${server.url}?${query}`);
    console.log(`measuring on ${machine}${headless ? " (headless, software rendering: a smoke run)" : ""}; leave the window alone`);
    const deadline = Date.now() + timeoutMs;
    let lastLine = "";
    while (Date.now() < deadline) {
      try {
        result = await page.evaluate(() => window.gantt?.measurement ?? null);
        if (result) break;
        const line = await page.evaluate(() => (document.getElementById("overlay")?.textContent ?? "").split("\n").find((l) => l.startsWith("measure")) ?? "");
        if (line !== lastLine) console.log(line);
        lastLine = line;
      } catch (err) {
        // A WebGPU panic reloads the page on WebGL2, which ends the old execution context (any other WebGPU failure switches in place).
        if (!/context was destroyed|navigat/i.test(String(err))) throw err;
      }
      await new Promise((r) => setTimeout(r, 1000));
    }
    if (!result) throw new Error(`no result within ${timeoutMs / 60_000} minutes`);
    if (openFile !== undefined && !result.error) {
      console.log(`opening ${openFile} ${openRepeats + 1} times (the first is a warm-up); leave the window alone`);
      const open = await measureOpens(page, { file: openFile, repeats: openRepeats, project: openProject });
      result = { ...result, open: { ...open, verdict: openVerdict(result.environment, open) } };
    }
  } finally {
    try {
      await browser?.close();
    } finally {
      await server.stop();
    }
  }

  mkdirSync(out, { recursive: true });
  const slug = machine.replace(/[^A-Za-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40);
  const file = join(out, `measure-${new Date().toISOString().replace(/[:.]/g, "-")}-${slug}.json`);
  writeFileSync(file, JSON.stringify({ ...result, chromeArgs }, null, 2));
  console.log(`wrote ${file}`);
  if (result.error) {
    console.error(`the run failed: ${result.error}`);
    process.exit(1);
  }
  for (const w of result.warnings) console.log(`warning: ${w}`);
  for (const v of result.verdicts) console.log(`${v.verdict.padEnd(4)} ${v.name}: ${Number(v.value).toFixed(2)} ${v.unit} (limit ${Number(v.limit).toFixed(2)})`);
  console.log(`overall ${result.verdict}`);
  if (result.edit) {
    const { edit } = result;
    const ms = (value) => `${Number(value).toFixed(2)} ms`;
    console.log(`edit task ${edit.task} (row ${edit.row}, set to ${edit.text}), ${edit.moved} rows moved, ${edit.movedPerRepeat.length} edits: p95 ${ms(edit.afterEdit.p95)} after an edit, ${ms(edit.noEdit.p95)} with none; encode p95 ${ms(edit.afterEdit.encodeMs.p95)} and ${ms(edit.noEdit.encodeMs.p95)}`);
  }
  if (result.linkEdit) {
    // A result written before gs#725 has no `linkEdit`.
    const { linkEdit: link } = result;
    const ms = (value) => `${Number(value).toFixed(2)} ms`;
    console.log(`link edit ${link.pred} -> ${link.task} (rows ${link.predRow}, ${link.row}), ${link.moved} rows moved, ${link.movedPerRepeat.length} edits: p95 ${ms(link.afterEdit.p95)} after an add, ${ms(link.noEdit.p95)} with none; add took p95 ${ms(link.setMs.p95)}${link.copyMs ? `, copy p95 ${ms(link.copyMs.p95)}` : ""}`);
  }
  if (result.open) {
    const { open } = result;
    const ms = (value) => `${Number(value).toFixed(1)} ms`;
    console.log(`open ${open.file} (${open.bytes} bytes, ${open.tasks} tasks), ${open.repeats.length} opens after a warm-up: p50 ${ms(open.p50)}, p95 ${ms(open.p95)}`);
    console.log(`  p50 by phase: ${OPEN_PHASES.filter((p) => p !== "total").map((p) => `${p} ${ms(open.phases[p].p50)}`).join(", ")}`);
    const v = open.verdict;
    if (v.verdict) console.log(`${v.verdict.padEnd(4)} ${v.name}: ${Number(v.value).toFixed(2)} ${v.unit} (limit ${Number(v.limit).toFixed(2)})`);
    else console.log(`open: ${v.note}`);
  }
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main().catch(async (err) => {
    if (stopping) await stopping;
    else throw err;
  });
}
