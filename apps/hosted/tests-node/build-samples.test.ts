import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { CAP_ENV, MAX_BYTES, capFor, checkSize } from "../scripts/sample-cap.ts";
import { CREWS, activities, assignedTasks, toMspdi } from "../samples/office-fitout.ts";
import { readSamplesJson } from "../tests/samples-file.ts";

// The samples build fails loudly (gs#561), checked in Node (moved from tests/build-samples.spec.ts). The first test
// checks that a missing `samples.json` names the script that writes it. The rest check the 25 MiB cap as a function and
// run the real `build-samples.mjs` with the cap lowered to see the build stop, in a temporary copy, so they never touch
// the `public/samples/` the browser specs read.

const HOSTED = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT_NAME = "apps/hosted/scripts/build-samples.mjs";

/** A fresh temporary directory, removed after the test. */
function scratch(): string {
  return mkdtempSync(join(tmpdir(), "hosted-561-"));
}

test("a missing samples.json throws an error that names build-samples.mjs", () => {
  const dir = scratch();
  try {
    expect(() => readSamplesJson(pathToFileURL(`${dir}/`))).toThrow(SCRIPT_NAME);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("capFor is 25 MiB, lowered by the environment, and never raised by it", () => {
  expect(MAX_BYTES).toBe(25 * 1024 * 1024);
  expect(capFor({})).toBe(MAX_BYTES);
  expect(capFor({ [CAP_ENV]: "" })).toBe(MAX_BYTES);
  expect(capFor({ [CAP_ENV]: "100" })).toBe(100);
  expect(capFor({ [CAP_ENV]: String(MAX_BYTES) })).toBe(MAX_BYTES);
  for (const bad of [String(MAX_BYTES + 1), "99999999999", "0", "-5", "1e3", "1.5", "ten"]) {
    expect(() => capFor({ [CAP_ENV]: bad }), `${CAP_ENV}=${bad}`).toThrow(CAP_ENV);
  }
});

test("checkSize passes below the cap and throws, naming the file, at the cap and over it", () => {
  expect(() => checkSize("a.xml.gz", MAX_BYTES - 1, MAX_BYTES)).not.toThrow();
  expect(() => checkSize("big.xer.gz", MAX_BYTES, MAX_BYTES)).toThrow(`big.xer.gz is ${MAX_BYTES} bytes, at or over the ${MAX_BYTES} byte cap`);
  expect(() => checkSize("big.xer.gz", MAX_BYTES + 1, MAX_BYTES)).toThrow("big.xer.gz");
  expect(() => checkSize("small.xml.gz", 101, 100)).toThrow("small.xml.gz");
});

test("build-samples.mjs stops at a file over a lowered cap, names it, and leaves nothing behind", () => {
  const dir = scratch();
  try {
    // The script finds the repo root and its output directory from its own location, so run a copy of it and of
    // `samples/` under a temporary `apps/hosted/`. The first sample it packs is the office fit-out, which needs no cargo.
    const copy = join(dir, "apps", "hosted");
    mkdirSync(copy, { recursive: true });
    cpSync(join(HOSTED, "scripts"), join(copy, "scripts"), { recursive: true });
    cpSync(join(HOSTED, "samples"), join(copy, "samples"), { recursive: true });
    const run = spawnSync(process.execPath, [join(copy, "scripts", "build-samples.mjs")], {
      encoding: "utf8",
      env: { ...process.env, [CAP_ENV]: "100", CARGO: join(dir, "no-cargo-should-run") },
    });
    expect(run.stderr).toContain("office-fitout.xml.gz is ");
    expect(run.stderr).toContain("at or over the 100 byte cap");
    expect(run.status, "the build exits 1").toBe(1);
    expect(run.stdout, "no sample was written").not.toContain("wrote");
    expect(existsSync(join(copy, "public", "samples")), "no half-built samples directory").toBe(false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the fit-out file names its crews as resources and assigns one to every task that is not a milestone", () => {
  const xml = toMspdi();
  const resources = new Map([...xml.matchAll(/<Resource><UID>(\d+)<\/UID><ID>\d+<\/ID><Name>([^<]+)<\/Name><\/Resource>/g)].map((m) => [m[1], m[2]]));
  const assignments = [...xml.matchAll(/<Assignment><UID>\d+<\/UID><TaskUID>(\d+)<\/TaskUID><ResourceUID>(\d+)<\/ResourceUID><\/Assignment>/g)].map((m) => ({ task: m[1]!, resource: m[2]! }));
  expect(resources.size, "a resource for each crew").toBe(Object.keys(CREWS).length);
  expect(assignments.every((a) => resources.has(a.resource)), "every assignment names a listed resource").toBe(true);
  const leaves = activities().filter((row) => row.milestone !== true);
  expect([...new Set(assignments.map((a) => a.task))].sort(), "the tasks with a crew").toEqual(leaves.map((row) => String(row.uid)).sort());
  expect(assignedTasks(), "the count the build marks as an expected note").toBe(leaves.length);
});

test("build-samples.mjs refuses a cap above 25 MiB instead of raising its limit", () => {
  const dir = scratch();
  try {
    const copy = join(dir, "apps", "hosted");
    mkdirSync(copy, { recursive: true });
    cpSync(join(HOSTED, "scripts"), join(copy, "scripts"), { recursive: true });
    cpSync(join(HOSTED, "samples"), join(copy, "samples"), { recursive: true });
    const run = spawnSync(process.execPath, [join(copy, "scripts", "build-samples.mjs")], {
      encoding: "utf8",
      env: { ...process.env, [CAP_ENV]: String(MAX_BYTES * 4), CARGO: join(dir, "no-cargo-should-run") },
    });
    expect(run.status).toBe(1);
    expect(run.stderr).toContain(CAP_ENV);
    expect(existsSync(join(copy, "public", "samples"))).toBe(false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
