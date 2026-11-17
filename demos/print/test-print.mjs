// The print demo (demos/print/run.sh) on the 5,000-task schedule: the PDF exists and holds two pages. The pages are the
// `/Type /Page` objects; the page tree is `/Type /Pages` and is not counted. Needs the build, Playwright's Chromium and a free
// GANTT_PORT, so it runs in CI's `web` job after the Chromium install, not with the browserless demo tests.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const run = join(dirname(fileURLToPath(import.meta.url)), "run.sh");
const out = mkdtempSync(join(tmpdir(), "print-demo-"));
try {
  execFileSync("bash", [run, "--out", out], { stdio: "inherit" });
  const pdf = join(out, "print.pdf");
  assert.ok(existsSync(pdf), "run.sh wrote print.pdf");
  const pages = readFileSync(pdf, "latin1").match(/\/Type\s*\/Page(?![A-Za-z])/g) ?? [];
  assert.equal(pages.length, 2, "the printed PDF holds two pages");
  console.log("ok: print.pdf holds two pages");
} finally {
  rmSync(out, { recursive: true, force: true });
}
