// Runs the Playwright specs with GANTT_URL, GANTT_PORT, CHROME, BACKEND, DPR, and N knobs.
// The `web` CI job runs this suite and nothing else against the page.
import { defineConfig } from "@playwright/test";
import { chromePath, dpr, port, url } from "./tests/support/knobs";

export default defineConfig({
  testDir: "./tests",
  testMatch: "**/*.spec.ts",
  // Three workers: a file's specs stay in order on one worker (fullyParallel is off); files run side by side on the one preview server (gs#1106).
  workers: 3,
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
    deviceScaleFactor: dpr,
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
          // Never reuse: a server already on the port is the CI runner's or another checkout's
          // build, and the specs would test that build, not this tree (gs#378).
          reuseExistingServer: false,
          timeout: 60_000,
        },
      }),
});
