// The launch page's code samples (apps/site/launch.html, tl#1048) compile against the chart's own types: each `<pre><code>` sample is wrapped in a
// file that imports the package by its name as `src/index.ts` (no `pnpm build:lib`), and `tsc --noEmit` reports an error in the sample, such as a method or filter
// name the chart does not have. Only errors in the sample files count; the package's own sources are checked by `pnpm typecheck`.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { PRODUCT_NAME } from "../src/product.ts";

const pkg = PRODUCT_NAME.toLowerCase();
const at = (path: string): string => fileURLToPath(new URL(path, import.meta.url));
const decode = (html: string): string => html.replace(/<[^>]+>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");

// Samples that are not TypeScript against this package, by their first line.
const SKIPPED: [string, string][] = [
  [`import { Gantt } from "${pkg}/react";`, "JSX, and its `Gantt` is the component, which the file this test wraps it in already names as the chart type"],
  ["/* your product's palette */", "CSS, not TypeScript"],
];
// Errors the page's samples have today, by message, each with the reason it stands; the test also fails when one of these stops occurring.
const KNOWN: [string, string][] = [];
// Names a sample uses without declaring, standing for the host's own code; each is declared in the wrapper only when the sample does not declare it.
const HOST: [string, string][] = [
  ["chart", "declare const chart: Gantt;"],
  ["file", "declare const file: File;"],
  ["save", "declare function save(document: unknown): void;"],
  ["toolbar", "declare const toolbar: { render(...shown: unknown[]): void };"],
];

test("every_typescript_sample_on_the_launch_page_compiles_against_the_chart_types", () => {
  const html = readFileSync(at("../../../apps/site/launch.html"), "utf8");
  const samples = [...html.matchAll(/<pre><code>([\s\S]*?)<\/code><\/pre>/g)].map((m) => decode(m[1]!));
  expect(samples.length, "the page has code samples").toBeGreaterThan(0);
  for (const [first] of SKIPPED) expect(samples.filter((s) => s.startsWith(first)), `one sample starts ${first}`).toHaveLength(1);
  const compiled = samples.filter((s) => !SKIPPED.some(([first]) => s.startsWith(first)));
  expect(compiled.length, "some sample is compiled").toBeGreaterThan(0);
  const dir = mkdtempSync(at("../node_modules/.samples-")); // inside the package, so the config's `types` (vite/client, node) resolve; node_modules is ignored by git
  try {
    compiled.forEach((sample, i) => {
      const hostNames = HOST.filter(([name]) => !new RegExp(`\\b(const|function) ${name}\\b`).test(sample)).map(([, line]) => line);
      writeFileSync(join(dir, `sample-${i}.ts`), `import type { Gantt } from "${pkg}";\n${hostNames.join("\n")}\n${sample}\n`);
    });
    const config = { extends: at("../tsconfig.json"), compilerOptions: { paths: { [pkg]: [at("../src/index.ts")] }, noUnusedLocals: false }, include: ["./sample-*.ts"] };
    writeFileSync(join(dir, "tsconfig.json"), JSON.stringify(config));
    const run = spawnSync(at("../node_modules/.bin/tsc"), ["-p", dir, "--noEmit", "--pretty", "false"], { encoding: "utf8" });
    expect(run.error, "tsc ran").toBeUndefined();
    const inSamples = run.stdout.split("\n").filter((line) => /sample-\d+\.ts\(/.test(line)).map((line) => line.slice(line.search(/sample-\d+\.ts\(/)));
    for (const [message] of KNOWN) expect(inSamples.filter((line) => line.includes(message)), `a sample still has: ${message}`).toHaveLength(1);
    expect(inSamples.filter((line) => !KNOWN.some(([message]) => line.includes(message))), "no other type error in a sample").toEqual([]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
