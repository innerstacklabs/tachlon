// Every method of the `Gantt` interface (src/gantt-types.ts) is named as a call, `name(`, in the package README or AGENTS.md. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const pkg = (file: string): string => readFileSync(new URL(`../${file}`, import.meta.url), "utf8");

test("every_gantt_method_is_named_in_the_readme_or_agents_md", () => {
  const body = /^export interface Gantt[^{]*\{([\s\S]*?)^\}/m.exec(pkg("src/gantt-types.ts"))?.[1] ?? "";
  const methods = [...body.matchAll(/^ {2}(\w+)\(/gm)].map((m) => m[1]!);
  expect(methods.length, "the interface's methods are found").toBeGreaterThanOrEqual(18);
  const docs = pkg("README.md") + pkg("AGENTS.md");
  const missing = methods.filter((name) => !new RegExp(`\\b${name}\\(`).test(docs));
  expect(missing, "methods named in neither the README nor AGENTS.md").toEqual([]);
});
