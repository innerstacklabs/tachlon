// The chart's words by key (gantt#1875): `chart.html` and `TEXTS_EN` name the same keys, so a typed key never leaves an element empty.
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";
import { FIELDS_EN, TEXTS_EN } from "../src/texts-en.ts";

test("every key in chart.html has an English text, and every English text is used", () => {
  const html = readFileSync(new URL("../src/chart.html", import.meta.url), "utf8");
  const used = new Set([...html.matchAll(/ data-t(?:-title|-aria-label|-placeholder)?="([^"]+)"/g)].map((m) => m[1]));
  expect([...used].filter((key) => !(key in TEXTS_EN)), "keys with no English text").toEqual([]);
  expect(Object.keys(TEXTS_EN).filter((key) => !used.has(key)), "English texts no element uses").toEqual([]);
});

// The names the script writes (`field-names.ts`): a key typed at a `nameOf` call has an English text, and every `FIELDS_EN` key is named in the script.
test("every name the script asks for has an English text, and every FIELDS_EN key is named", () => {
  const files = readdirSync(new URL("../src/", import.meta.url)).filter((f) => f.endsWith(".ts") && !["texts-en.ts", "field-names.ts"].includes(f));
  const code = files.map((f) => readFileSync(new URL(`../src/${f}`, import.meta.url), "utf8")).join("\n");
  const asked = [...code.matchAll(/\bnameOf\([^,]+, "([^"]+)"\)|\bn\("([^"]+)"\)/g)].map((m) => m[1] ?? m[2]!);
  expect(asked.length, "the scan finds the calls").toBeGreaterThan(10);
  expect(asked.filter((key) => !(key in TEXTS_EN) && !(key in FIELDS_EN)), "keys with no English text").toEqual([]);
  expect(Object.keys(FIELDS_EN).filter((key) => !code.includes(`"${key}"`)), "FIELDS_EN keys no code names").toEqual([]);
});
