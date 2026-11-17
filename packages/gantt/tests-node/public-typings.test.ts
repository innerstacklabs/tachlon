// What the package makes public to a developer's editor (tl#1089): the typings the build emits (`scripts/emit-types.ts`) hold the
// entry's API and no generated wasm type, and compile for a consumer that checks every `.d.ts` (`skipLibCheck` off).
import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { expect } from "@playwright/test";

const at = (path: string): string => fileURLToPath(new URL(path, import.meta.url));

test("the emitted typings hold the entry's API, name no wasm type, and compile for a consumer", () => {
  const out = mkdtempSync(join(tmpdir(), "gantt-typings-"));
  try {
    execFileSync("node", [at("../../../scripts/emit-types.ts"), at(".."), out], { stdio: "pipe" });
    const files = readdirSync(out, { recursive: true, encoding: "utf8" }).filter((f) => f.endsWith(".d.ts"));
    const text = (f: string): string => readFileSync(join(out, f), "utf8");
    expect(files.filter((f) => /\/wasm\/|\.css"|\b(InitOutput|PublicApp|GanttHook|HostInit|createApp|installTestHook)\b/.test(text(f)))).toEqual([]);
    expect(text("mount-load.d.ts")).toContain("export declare function preloadEngine(options?: Pick<MountOptions, \"wasmUrl\">): Promise<void>;");
    expect(text("index.d.ts")).toContain("export declare function mount(root: HTMLDivElement, options: MountOptions): Promise<Gantt>;");
    // A consumer's compile: no tsconfig, no @types, only the emitted files.
    const tsc = ["--noEmit", "--strict", "--target", "es2022", "--module", "esnext", "--moduleResolution", "bundler", "--lib", "es2022,dom", "index.d.ts"];
    execFileSync(at("../node_modules/.bin/tsc"), tsc, { cwd: out, stdio: "pipe" });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});
