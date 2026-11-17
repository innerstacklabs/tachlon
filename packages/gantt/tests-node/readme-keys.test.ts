// Every letter or symbol key that `src/keys.ts` handles, and Home, End, PageUp and PageDown, has a row in the root README keys table. Read as text.
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");

test("every_letter_or_symbol_key_the_chart_handles_has_a_row_in_the_readme_keys_table", () => {
  const keys = [...new Set([...read("../src/keys.ts").matchAll(/e\.key === "([^"]|Home|End|PageUp|PageDown)"/g)].map((m) => m[1]!))];
  expect(keys.length, "the handled keys are found").toBeGreaterThanOrEqual(15);
  const readme = read("../../../README.md");
  const table = readme.slice(readme.indexOf("<!-- ANCHOR: keys -->"), readme.indexOf("<!-- ANCHOR_END: keys -->"));
  // The first column of each row: its code spans, and the key after a `Ctrl+` or `Shift+` in one.
  const named = new Set<string>();
  for (const row of table.split("\n").filter((line) => line.startsWith("| `"))) {
    for (const [, span] of row.split(" | ")[0]!.matchAll(/`([^`]+)`/g)) named.add(span!).add(span!.replace(/^.+\+(?=.)/, ""));
  }
  const missing = keys.filter((k) => !named.has(/^[A-Z]$/.test(k) ? `Shift+${k}` : k === " " ? "Space" : k));
  expect(missing, "keys handled in src/keys.ts with no row in the README keys table").toEqual([]);
});
