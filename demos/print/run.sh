#!/usr/bin/env bash
# Print the 5,000-task schedule to a PDF: opens the demo page at ?n=5000, turns on the Longest path filter and saves the
# printed pages as <out>/print.pdf (US Letter landscape, one SVG page per sheet). demos/print/test-print.mjs runs it in CI.
#   demos/print/run.sh [--out <dir>]   default <dir>: a new temporary folder; the last line names the PDF
# Needs Playwright's Chromium (`pnpm exec playwright install chromium` in packages/gantt). Builds the page first when
# packages/gantt/dist is missing. GANTT_PORT picks the preview port (default 4173).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
out="$(mktemp -d)"
[[ "${1:-}" != "--out" ]] || out="${2:?run.sh: --out needs a folder}"
mkdir -p "$out"
[[ -f "$root/packages/gantt/dist/index.html" ]] || bash "$root/scripts/build-web.sh"
export PRINT_URL="http://127.0.0.1:${GANTT_PORT:-4173}/" PRINT_PDF="$(realpath -- "$out")/print.pdf" PRINT_ROOT="$root"
cd "$root/packages/gantt"
pnpm exec vite preview --host 127.0.0.1 --port "${GANTT_PORT:-4173}" --strictPort >"$out/preview.log" 2>&1 &
trap 'kill $! 2>/dev/null || true' EXIT
node --input-type=module - <<'JS'
import { createRequire } from "node:module";
const { chromium } = createRequire(`${process.env.PRINT_ROOT}/packages/gantt/package.json`)("@playwright/test");
const url = process.env.PRINT_URL;
for (let i = 0; i < 120 && !(await fetch(url).then((r) => r.ok, () => false)); i++) await new Promise((r) => setTimeout(r, 500));
const browser = await chromium.launch({ args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  await page.goto(`${url}?n=5000&backend=webgl`);
  await page.waitForFunction(() => window.gantt?.ready && window.gantt.status().frames >= 5, null, { timeout: 180_000 });
  await page.keyboard.press("L"); // the Longest path key, as the toolbar button
  await page.waitForFunction(() => window.gantt.status().filter === "longest");
  // 26 weeks of the longest path from the page's default window start (the data date; this fixture has none, so the project start): 40 rows, two sheets (the whole path is 333 rows, 14 sheets).
  await page.evaluate(() => window.gantt.app.set_lookahead(window.gantt.status().lookahead.start, 26));
  await page.waitForFunction(() => window.gantt.status().lookahead.on);
  // Paper is Letter landscape, as the hosted page.
  await page.addStyleTag({ content: "@page { size: letter landscape; margin: 0; }" });
  await page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  const sheets = await page.locator(".gantt-print-pages > svg").count();
  await page.pdf({ path: process.env.PRINT_PDF, preferCSSPageSize: true });
  console.log(`printed ${sheets} sheets to ${process.env.PRINT_PDF}`);
} finally {
  await browser.close();
}
JS
