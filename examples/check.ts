// Checks the examples (gantt#1092), each the way a stranger meets it: the example is copied into a new folder, the packed chart package is installed
// there as `tachlon` (the name both examples import; its `/react` entry is the React wrapper), the folder is served by its own Vite and its spec drives it in Chromium.
//   node examples/check.ts plain react
// With `--registry [<version>]` the package comes from npm instead (the version the example pins, unless one is given), the check for a published release:
//   node examples/check.ts --registry [<version>] plain react
import { execFileSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromePath } from "../packages/gantt/tests/support/knobs.ts";
import { launchRetrying } from "../scripts/lib/launch-retrying.ts";
import { WEB_ROOT } from "../scripts/lib/web-root.ts";

const CHART = resolve(import.meta.dirname, "..", WEB_ROOT);
const run = (cwd: string, cmd: string, args: string[]): string => execFileSync(cmd, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

// Runs the spec of the example `name` on a copy of it with the packed package installed (`registry`: the published one, `version` or the pinned one). Throws on the first problem.
async function check(name: string, registry: boolean, version?: string): Promise<void> {
  const work = mkdtempSync(join(tmpdir(), `example-${name}-`));
  try {
    const app = join(work, "app");
    cpSync(join(import.meta.dirname, name), app, { recursive: true });
    const [packed] = registry ? [] : JSON.parse(run(CHART, "npm", ["pack", "--json", "--ignore-scripts", "--pack-destination", work]));
    const pinned = JSON.parse(readFileSync(join(app, "package.json"), "utf8")).dependencies.tachlon;
    run(app, "npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund", registry ? `tachlon@${version ?? pinned}` : `tachlon@file:${join(work, packed.filename)}`]);
    const site = await createRequire(join(app, "package.json"))("vite").createServer({ root: app, logLevel: "error", server: { host: "127.0.0.1", port: 0 } });
    try {
      await site.listen();
      const { chromium } = createRequire(join(CHART, "package.json"))("@playwright/test") as typeof import("@playwright/test");
      const browser = await launchRetrying(() => chromium.launch({ executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] }));
      try {
        const spec = (await import(join(import.meta.dirname, name, "spec.ts"))).default as (page: import("@playwright/test").Page, url: string) => Promise<void>;
        await spec(await browser.newPage(), site.resolvedUrls.local[0]);
      } finally {
        await browser.close();
      }
    } finally {
      await site.close(); // every path after the server exists ends here, so a failed Chromium launch cannot leave Vite holding the process open
    }
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

const args = process.argv.slice(2);
const at = args.indexOf("--registry");
const version = at >= 0 && /^\d/.test(args[at + 1] ?? "") ? args[at + 1] : undefined;
const names = args.filter((arg, i) => i !== at && !(version !== undefined && i === at + 1));
if (names.length === 0) {
  console.error("usage: node examples/check.ts [--registry [<version>]] <example folder>...");
  process.exit(2);
}
for (const name of names) {
  try {
    await check(name, at >= 0, version);
    console.log(`examples: PASS ${name}`);
  } catch (e) {
    console.error(`examples: FAIL ${name}: ${(e as Error).message.split("\n").slice(0, 6).join("\n")}`);
    process.exitCode = 1;
  }
}
