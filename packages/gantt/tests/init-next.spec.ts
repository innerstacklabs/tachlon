import { execFileSync, spawn } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { chromePath } from "./support/knobs";
// tl#1249 row 4: a stranger's Next app gets the chart from one command. The package is packed, `init` runs in a copy of the fixture app (installing the tarball),
// `next build` compiles the component it wrote, and Chromium loads the built app: the chart mounts with the engine fetched from /app_bg.wasm. A second `init` changes no byte.
const CHART = fileURLToPath(new URL("..", import.meta.url));
const FIXTURE = fileURLToPath(new URL("./fixtures/next-app", import.meta.url));
const NAME: string = JSON.parse(readFileSync(join(CHART, "package.json"), "utf8")).name; // the product's name is written in one place only
const run = (cwd: string, cmd: string, args: string[]): string => execFileSync(cmd, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" } });
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });

// Every file of the app outside node_modules and .next, with its bytes.
const bytes = (app: string): Record<string, string> =>
  Object.fromEntries(readdirSync(app, { recursive: true, withFileTypes: true }).filter((e) => e.isFile() && !/(^|\/)(node_modules|\.next)(\/|$)/.test(join(e.parentPath, e.name).slice(app.length))).map((e) => [join(e.parentPath, e.name), readFileSync(join(e.parentPath, e.name)).toString("hex")]));
const freePort = (): Promise<number> => new Promise((done) => { const s = createServer().listen(0, "127.0.0.1", () => { const { port } = s.address() as { port: number }; s.close(() => done(port)); }); });

test("init adds the chart to a Next app: the packed package installs, the app builds, the chart mounts, and a second run changes nothing", async ({ page }) => {
  test.setTimeout(900_000);
  if (!existsSync(join(CHART, "dist-lib/cli/main.js"))) run(CHART, "pnpm", ["build:lib"]);
  const work = mkdtempSync(join(tmpdir(), "init-next-"));
  let server: ReturnType<typeof spawn> | undefined;
  try {
    const [packed] = JSON.parse(run(CHART, "npm", ["pack", "--json", "--ignore-scripts", "--pack-destination", work]));
    const app = join(work, "app");
    cpSync(FIXTURE, app, { recursive: true });
    run(app, "npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund"]);
    const init = () => run(app, "node", [join(CHART, "dist-lib/cli/main.js"), "init", "--spec", `file:${join(work, packed.filename)}`]);
    expect(init()).toContain(`wrote: public/app_bg.wasm`);
    const done = bytes(app);
    expect(init()).toContain("already set up, nothing changed");
    expect(bytes(app)).toEqual(done);
    writeFileSync(join(app, "app/page.jsx"), `import Chart from "../components/${NAME}-chart";\nexport default function Page() {\n  return <Chart />;\n}\n`);
    run(app, "npx", ["next", "build"]);

    const port = await freePort();
    server = spawn("node", [join(app, "node_modules/next/dist/bin/next"), "start", "-p", String(port), "-H", "127.0.0.1"], { cwd: app, stdio: "ignore", detached: true });
    const base = `http://127.0.0.1:${port}/`;
    await expect.poll(() => fetch(base).then((r) => r.status, () => 0), { timeout: 60_000 }).toBe(200);
    const problems: string[] = [];
    page.on("pageerror", (e) => problems.push(e.message));
    page.on("console", (m) => m.type() === "error" && !m.text().startsWith("Failed to load resource") && problems.push(m.text()));
    page.on("response", (r) => r.status() >= 400 && !r.url().endsWith("/favicon.ico") && problems.push(`${r.status()} ${r.url()}`)); // the app has no icon
    const engine = page.waitForResponse((r) => new URL(r.url()).pathname === "/app_bg.wasm");
    await page.goto(base);
    expect((await engine).status()).toBe(200);
    await page.locator('#table-rows .row[data-row="2"]').waitFor({ timeout: 60_000 });
    expect(problems).toEqual([]);
  } finally {
    if (server?.pid) process.kill(-server.pid);
    rmSync(work, { recursive: true, force: true });
  }
});
