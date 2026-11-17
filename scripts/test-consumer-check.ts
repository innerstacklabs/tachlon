// Checks the consumer check (see scripts/lib/consumer-check.ts) on two temporary fixture packages, without a browser: a good one that
// declares everything its `exports` names, and a bad one whose tarball leaves out the declaration file. Needs the chart folder's
// `pnpm install` (the check type-checks with its TypeScript). Exits 1, one FAIL line per problem.
//   node scripts/test-consumer-check.ts
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { consumerProblems } from "./lib/consumer-check.ts";

let failed = false;
const check = (name: string, got: string[], want: (problems: string[]) => boolean): void => {
  if (want(got)) return;
  failed = true;
  console.error(`test-consumer-check: FAIL ${name}: got ${JSON.stringify(got)}`);
};

const root = mkdtempSync(join(tmpdir(), "test-consumer-check-"));
// A package that declares `mount` and ships `files`; its `exports` names the script and the declarations.
function fixture(name: string, files: string[]): string {
  const dir = join(root, name);
  mkdirSync(join(dir, "dist"), { recursive: true });
  writeFileSync(join(dir, "dist/index.js"), "export const mount = () => Promise.resolve({ state: () => ({ taskCount: 1 }) });\n");
  writeFileSync(join(dir, "dist/index.d.ts"), "export interface MountOptions { tasks?: unknown[]; forceWebgl?: boolean; wasmUrl?: string }\nexport declare function mount(root: HTMLDivElement, options: MountOptions): Promise<{ state(): { taskCount: number } }>;\n");
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name, version: "1.0.0", type: "module", files, exports: { ".": { types: "./dist/index.d.ts", default: "./dist/index.js" } } }));
  return dir;
}

try {
  // The good package installs and its page type-checks.
  check("good package", await consumerProblems(fixture("good", ["dist"]), { browser: false }), (p) => p.length === 0);
  // The bad package's tarball lacks the declaration file its `exports` names: the page fails to compile, and the problem says so.
  check("bad package", await consumerProblems(fixture("bad", ["dist/index.js"]), { browser: false }), (p) => p.length > 0 && p.some((line) => line.includes("TS7016")));
  // A compiler that cannot run (here a stub that prints only to stderr and exits 3) is a failure that says so, never a pass.
  const stub = join(root, "tsc-stub.js");
  writeFileSync(stub, 'console.error("tsc: cannot start"); process.exit(3);\n');
  check("compiler that fails with no diagnostics", await consumerProblems(fixture("stubbed", ["dist"]), { browser: false, tsc: stub }), (p) => p.length === 1 && p[0]!.includes("exit 3") && p[0]!.includes("tsc: cannot start"));
} finally {
  rmSync(root, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("test-consumer-check: PASS");
