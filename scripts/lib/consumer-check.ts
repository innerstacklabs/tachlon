// The consumer check (tl#1089 L6): the package as a stranger gets it. Packs the package folder, installs the tarball into an empty folder,
// type-checks a small page against its declarations (the compile errors a consumer would see), and with a browser serves that folder and
// loads the chart on the page until it is ready, the engine file named by `wasmUrl` and found inside the installed package.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { extname, join, resolve } from "node:path";
import { chromePath } from "../../packages/gantt/tests/support/knobs.ts";
import { launchRetrying } from "./launch-retrying.ts";
import { WEB_ROOT } from "./web-root.ts";

const CHART = resolve(import.meta.dirname, "../..", WEB_ROOT);
const TYPES = { ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".html": "text/html", ".woff2": "font/woff2" } as Record<string, string>;
const run = (cwd: string, cmd: string, args: string[]): string => execFileSync(cmd, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

// The page a consumer writes: the public entry only, the engine named by `wasmUrl`, a flag on the body once the chart has started (or why not).
const page = (name: string): string => `import { mount, type MountOptions } from "${name}";
const options: MountOptions = { tasks: [{ id: 1, name: "a", start: "2026-11-02", duration: 3 }], forceWebgl: true, wasmUrl: "./node_modules/${name}/dist-lib/app_bg.wasm" };
mount(document.getElementById("g") as HTMLDivElement, options).then(
  (g) => { document.body.dataset.ready = String(g.state().taskCount); },
  (e: { stage?: string }) => { document.body.dataset.failed = e.stage ?? String(e); },
);
`;

// Packs `dir` and installs the tarball into a new empty folder inside `work`; returns that folder and the package's name.
function install(dir: string, work: string): { box: string; name: string } {
  const [packed] = JSON.parse(run(dir, "npm", ["pack", "--json", "--ignore-scripts", "--pack-destination", work]));
  const box = join(work, "box");
  mkdirSync(box);
  writeFileSync(join(box, "package.json"), '{"private":true,"type":"module"}\n');
  run(box, "npm", ["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", join(work, packed.filename)]);
  return { box, name: JSON.parse(readFileSync(join(dir, "package.json"), "utf8")).name };
}

// The compile errors of the page against the installed declarations, one line each (`skipLibCheck` is off: the declarations are checked too).
function compileErrors(box: string, name: string, tsc: string): string[] {
  writeFileSync(join(box, "page.ts"), page(name));
  const options = { target: "ES2022", module: "ESNext", moduleResolution: "bundler", lib: ["ES2022", "DOM"], types: [], strict: true, outDir: "out" };
  writeFileSync(join(box, "tsconfig.json"), JSON.stringify({ compilerOptions: options, files: ["page.ts"] }));
  try {
    run(box, "node", [tsc, "--project", "tsconfig.json"]);
    return [];
  } catch (e) {
    const { stdout, stderr, status } = e as { stdout?: string; stderr?: string; status?: number | null };
    const lines = String(stdout ?? "").split("\n").filter((line) => line.includes("error TS"));
    // A compiler that failed without one diagnostic (it could not start, or crashed) is a failure, never a pass.
    return lines.length ? lines : [`the compiler failed (exit ${status ?? "none"}) with no error TS line: ${String(stderr || e).trim().split("\n")[0]}`];
  }
}

// Serves the folder `box` on a free local port.
export const serve = (box: string): Promise<{ server: Server; url: string }> => new Promise((done) => {
  const server = createServer((req, res) => {
    try {
      const path = new URL(req.url ?? "/", "http://x").pathname.replace(/\/$/, "/index.html");
      const body = readFileSync(join(box, path));
      res.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" }).end(body);
    } catch {
      res.writeHead(404).end("not found");
    }
  });
  server.listen(0, "127.0.0.1", () => done({ server, url: `http://127.0.0.1:${(server.address() as { port: number }).port}/` }));
});

// Loads the chart on a page served from the installed box. Returns a problem line, or none once the chart is ready with its one task.
async function loadChart(box: string, name: string): Promise<string[]> {
  const exp = JSON.parse(readFileSync(join(box, "node_modules", name, "package.json"), "utf8")).exports;
  const at = (target: string): string => `./node_modules/${name}/${target.replace(/^\.\//, "")}`;
  writeFileSync(join(box, "index.html"), `<link rel="stylesheet" href="${at(exp["./gantt.css"])}"><div id="g" style="width:800px;height:300px"></div>
<script type="importmap">{"imports":{"${name}":"${at(exp["."].default)}"}}</script><script type="module" src="./out/page.js"></script>`);
  const { chromium } = createRequire(join(CHART, "package.json"))("@playwright/test") as typeof import("@playwright/test");
  const browser = await launchRetrying(() => chromium.launch({ executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] }));
  const { server, url } = await serve(box);
  try {
    const tab = await browser.newPage();
    await tab.goto(url);
    await tab.waitForFunction(() => document.body.dataset.ready || document.body.dataset.failed, undefined, { timeout: 60_000 });
    const { ready, failed } = await tab.evaluate(() => ({ ready: document.body.dataset.ready, failed: document.body.dataset.failed }));
    return ready === "1" ? [] : [`the chart did not reach ready: ${failed ? `failed at ${failed}` : `${ready} tasks`}`];
  } catch (e) {
    return [`the chart did not reach ready: ${(e as Error).message.split("\n")[0]}`];
  } finally {
    server.close();
    await browser.close();
  }
}

// The problems a consumer of the package in `dir` meets, one line each; none means it passes. Throws when the package cannot be packed or installed.
export async function consumerProblems(dir: string, options: { browser: boolean; tsc?: string }): Promise<string[]> {
  const work = mkdtempSync(join(tmpdir(), "consumer-check-"));
  try {
    const { box, name } = install(resolve(dir), work);
    const errors = compileErrors(box, name, options.tsc ?? join(CHART, "node_modules/typescript/bin/tsc"));
    if (errors.length || !options.browser) return errors;
    return await loadChart(box, name);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}
