import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { expect } from "@playwright/test";

// The package's `exports` map names what programs may import; this reads it from the built package (`pnpm build:lib` in the CI job runs first), so a broken entry fails here before a publish.
const dir = new URL("../../../packages/gantt/", import.meta.url);
const exportsMap = (JSON.parse(readFileSync(new URL("package.json", dir), "utf8")) as { exports: Record<string, string | Record<string, string>> }).exports;
// These four need a page (a canvas, a window), so a Node test names them and checks only their files.
const PAGE_ONLY = [".", "./react", "./vue", "./angular"];

test("every_export_names_a_file_that_the_build_wrote", () => {
  for (const [entry, target] of Object.entries(exportsMap)) {
    for (const file of typeof target === "string" ? [target] : Object.values(target)) expect(existsSync(new URL(file, dir)), `${entry} -> ${file}`).toBe(true);
  }
  for (const entry of PAGE_ONLY) expect(exportsMap[entry], `${entry} is an export`).toBeDefined();
});

test("the_headless_and_schema_entries_load_in_node", async () => {
  const headless = await import("gantt-core/headless");
  expect(typeof headless.openHeadless).toBe("function");
  expect(typeof createRequire(import.meta.url)("gantt-core/schema.json")).toBe("object");
});

// The Angular entry is what `ngc` writes (gantt#1875), not a hand-written copy of its output: the script carries the partial declarations an app's build links, and the typings carry the component's `ɵcmp` type, which a template check reads.
test("the_angular_entry_is_what_ngc_wrote", () => {
  const read = (file: string): string => readFileSync(new URL(file, dir), "utf8");
  expect(read("dist-lib/angular.js")).toContain("ɵɵngDeclareComponent");
  expect(read("dist-lib/types/angular.d.ts")).toContain("ɵɵComponentDeclaration<Gantt, \"gantt-chart\"");
});
