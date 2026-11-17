import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { expect } from "@playwright/test";

// `Sample.format` stays in step with `scripts/build-samples.mjs` (gs#621). The script is plain JavaScript, so nothing
// typed wrote the `format` strings it puts in `samples.json`: removing "mspdi" from the union in `src/samples.ts` left
// `pnpm typecheck` green. The script now starts with `// @ts-check` and types its `samples` array as `Sample[]`, and
// `tsconfig.json` includes it. These tests keep that wiring from being undone. None opens a browser. Each runs the
// repo's own `tsc` on a temporary copy of the files, so the break never touches the tree the other specs read.

const HOSTED = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT = "build-samples.mjs";
const UNION = 'format: "xer" | "mspdi" | "demo";';
const MEMBERS = ["xer", "mspdi", "demo"];

/** Run `tsc --noEmit` on a temporary copy of the hosted files, with `samples.ts` rewritten by `edit`. */
function typecheckCopy(edit: (samples: string) => string): { status: number | null; output: string } {
  const dir = mkdtempSync(join(tmpdir(), "hosted-621-"));
  try {
    // The script reaches `../src/samples.ts` and `../samples/office-fitout.ts` from `scripts/`, so keep that layout.
    mkdirSync(join(dir, "src"));
    cpSync(join(HOSTED, "scripts"), join(dir, "scripts"), { recursive: true });
    cpSync(join(HOSTED, "samples"), join(dir, "samples"), { recursive: true });
    // The real tsconfig, so dropping the script from `include` or `allowJs` turns the first test red. Its `include`
    // names files this copy does not hold (the other specs, vite.config.ts); tsc skips a pattern that matches nothing.
    cpSync(join(HOSTED, "tsconfig.json"), join(dir, "tsconfig.json"));
    writeFileSync(join(dir, "src", "samples.ts"), edit(readFileSync(join(HOSTED, "src", "samples.ts"), "utf8")));
    symlinkSync(join(HOSTED, "node_modules"), join(dir, "node_modules"));
    const tsc = join(HOSTED, "node_modules", "typescript", "bin", "tsc");
    const run = spawnSync(process.execPath, [tsc, "--noEmit", "-p", dir], { cwd: dir, encoding: "utf8" });
    return { status: run.status, output: `${run.stdout}${run.stderr}` };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the copy typechecks as it is, so a failure below comes from the removed member", () => {
  expect(readFileSync(join(HOSTED, "src", "samples.ts"), "utf8"), "the union is written as this test expects").toContain(UNION);
  const run = typecheckCopy((samples) => samples);
  expect(run.output, "no error").toBe("");
  expect(run.status, "tsc exits 0").toBe(0);
});

for (const member of MEMBERS) {
  test(`removing "${member}" from Sample.format fails the typecheck of ${SCRIPT}`, () => {
    const rest = MEMBERS.filter((m) => m !== member)
      .map((m) => `"${m}"`)
      .join(" | ");
    const run = typecheckCopy((samples) => samples.replace(UNION, `format: ${rest};`));
    expect(run.status, "tsc exits non-zero").not.toBe(0);
    expect(run.output, `the error is in ${SCRIPT}`).toContain(`scripts/${SCRIPT}`);
    expect(run.output, `the error names "${member}"`).toContain(`"${member}"`);
  });
}
