import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { expect, test } from "@playwright/test";
import { SAMPLES_DIR, readSamplesJson } from "./samples-file";

// The samples build fails loudly (gs#561). None of these tests opens a browser. The first runs the real `samples.spec.ts`
// without `samples.json` in a temporary copy to see the run fail; the second reads the built medium-5k sample. The cap
// and the missing-file error are checked under `node --test` in tests-node/build-samples.test.ts.

const HOSTED = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT_NAME = "apps/hosted/scripts/build-samples.mjs";

/** A fresh temporary directory, removed after the test. */
function scratch(): string {
  return mkdtempSync(join(tmpdir(), "hosted-561-"));
}

test("playwright test fails, and says to run build-samples.mjs, when samples.json is missing", () => {
  const dir = scratch();
  try {
    // The real spec and its helper, in a project with no `public/samples/`; the browser and the web server are not needed to load them.
    mkdirSync(join(dir, "tests", "support"), { recursive: true });
    for (const name of ["samples.spec.ts", "samples-file.ts", "support/page.ts"]) cpSync(join(HOSTED, "tests", name), join(dir, "tests", name));
    for (const name of ["playwright.config.ts", "package.json"]) cpSync(join(HOSTED, name), join(dir, name));
    symlinkSync(join(HOSTED, "node_modules"), join(dir, "node_modules"));
    const cli = join(HOSTED, "node_modules", "@playwright", "test", "cli.js");
    const run = spawnSync(process.execPath, [cli, "test", "--list"], {
      cwd: dir,
      encoding: "utf8",
      env: { ...process.env, GANTT_URL: "http://127.0.0.1:1/" },
    });
    expect(`${run.stdout}${run.stderr}`).toContain(SCRIPT_NAME);
    expect(run.status, "the run exits non-zero").not.toBe(0);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("medium-5k's first activity is not named Activity", () => {
  // The generated samples carry construction names (gs#703), so the demo reads like a schedule. This reads the built
  // file's first TASK row, as `build-samples.mjs` wrote it; no browser is needed.
  const entry = readSamplesJson<{ samples: { id: string; file: string }[] }>().samples.find((sample) => sample.id === "medium-5k")!;
  const text = gunzipSync(readFileSync(new URL(entry.file, SAMPLES_DIR))).toString("latin1");
  const lines = text.split(/\r?\n/);
  const header = lines.findIndex((line) => line === "%T\tTASK") + 1;
  const name = lines[header]!.split("\t").indexOf("task_name") - 1;
  const first = lines[header + 1]!.split("\t").slice(1)[name]!;
  expect(first).not.toMatch(/^Activity/);
  expect(first).toContain(", level ");
});
