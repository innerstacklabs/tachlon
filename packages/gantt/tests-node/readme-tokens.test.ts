// The package README's "The stylesheet" section names every colour token of the Theming table in docs/WASM_API.md, and no other. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");
const section = (text: string, heading: string): string => text.split(`\n## ${heading}\n`)[1]?.split("\n## ")[0] ?? "";

test("the_readme_names_the_colour_tokens_of_the_theming_table", () => {
  const rows = section(read("../../../docs/WASM_API.md"), "Theming").matchAll(/^\| `([\w-]+)` \| `#[0-9a-f]+` \|/gm);
  const table = [...rows].map((m) => `--gantt-${m[1]}`).sort();
  expect(table.length, "the table's colour rows are found").toBeGreaterThanOrEqual(30);
  const named = section(read("../README.md"), "The stylesheet").match(/--gantt-[a-z-]+[a-z]/g) ?? [];
  expect([...new Set(named)].sort(), "the README's token names equal the table's").toEqual(table);
});
