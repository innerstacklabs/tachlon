// Runs the hosted specs against `vite preview` of this project's build, which is what the `hosted-browser` CI job does.
// GANTT_URL points at a server that is already running, GANTT_PORT picks the preview port, CHROME the browser.
import { readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { defineConfig } from "@playwright/test";

const port = Number(process.env.GANTT_PORT ?? 4300);
const url = process.env.GANTT_URL ?? `http://127.0.0.1:${port}/`;

/** The newest Chromium that `playwright install chromium` put under the cache, or `$CHROME`. */
function chromePath(): string {
  if (process.env.CHROME) return process.env.CHROME;
  const base = join(homedir(), ".cache", "ms-playwright");
  const dir = readdirSync(base)
    .filter((entry) => /^chromium-\d+$/.test(entry))
    .sort((left, right) => Number(left.slice("chromium-".length)) - Number(right.slice("chromium-".length)))
    .at(-1);
  if (!dir) throw new Error(`no Chromium under ${base}; set CHROME`);
  return join(base, dir, "chrome-linux64", "chrome");
}

export default defineConfig({
  testDir: "./tests",
  testMatch: "**/*.spec.ts",
  workers: 1,
  fullyParallel: false,
  // One retry: a spec that fails and then passes is reported as flaky in the run's summary line (gs#1106).
  retries: 1,
  forbidOnly: !!process.env.CI,
  // A test tagged @quarantine fails on main with no change behind it (gantt#1163): every CI run skips it except the nightly one, and a local run keeps it.
  grepInvert: process.env.CI && process.env.GITHUB_EVENT_NAME !== "schedule" ? /@quarantine/ : undefined,
  timeout: 240_000,
  expect: { timeout: 30_000 },
  reporter: [["list"], ["html", { open: "never" }]],
  projects: [{ name: "ci" }],
  use: {
    baseURL: url,
    viewport: { width: 1280, height: 800 },
    headless: true,
    launchOptions: {
      executablePath: chromePath(),
      args: [
        "--ignore-gpu-blocklist",
        "--use-angle=swiftshader",
        "--enable-unsafe-swiftshader",
        "--enable-unsafe-webgpu",
      ],
    },
  },
  ...(process.env.GANTT_URL
    ? {}
    : {
        webServer: {
          command: `pnpm preview --host 127.0.0.1 --port ${port} --strictPort`,
          url: `http://127.0.0.1:${port}/`,
          // Never reuse: a server already on the port is the CI runner's or another checkout's build, and the
          // spec would test that build, not this tree.
          reuseExistingServer: false,
          timeout: 60_000,
        },
      }),
});
