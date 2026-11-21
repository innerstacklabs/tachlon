// The import-notes section of the open guide file quotes the panel heading as `apps/chrome/src/unsupported.ts` writes it, and each note it quotes starts with
// words a format string in the importer or the loader writes (numbers and link kinds read as `{}`). Read as text.
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { expect } from "@playwright/test";

const root = new URL("../../../", import.meta.url);
const read = (path: string): string => readFileSync(new URL(path, root), "utf8");
const shape = (text: string): string => text.replace(/\{[^}]*\}|\b\d+\b|\b(?:FS|SS|FF|SF)\b/g, "{}").replace(/\{\}(, \{\})+/g, "{}");

test("the_book_quotes_the_panel_heading_and_the_notes_as_the_code_writes_them", () => {
  const section = read("packages/gantt/guide/open.md").split(/^## /m).find((part) => part.startsWith("What the panel says"));
  expect(section, "the section is found").toBeDefined();
  const [lead, ...items] = section!.split("\n- ");
  const heading = /"([^"\n]+)"/.exec(lead!)?.[1];
  const title = /unsupported-title"\)\.textContent = `\$\{total\}([^`$]*)/.exec(read("apps/chrome/src/unsupported.ts"))?.[1];
  expect(title, "the panel title is found in unsupported.ts").toBeDefined();
  expect(heading?.replace(/^\d+ import notes/, ""), "the heading the book quotes").toBe(title);
  const files = ["crates/import/src", "crates/app/src/load"].flatMap((dir) => readdirSync(new URL(dir, root), { recursive: true, encoding: "utf8" }).filter((name) => name.endsWith(".rs") && !/(^|\/)tests?(\.rs|\/)/.test(name)).map((name) => `${dir}/${name}`));
  const formats = [...files, "crates/app/src/load.rs"].flatMap((path) => [...read(path).matchAll(/"((?:[^"\\\n]|\\.)*)"/g)].map((m) => shape(m[1]!)));
  expect(formats.length, "the format strings are found").toBeGreaterThan(100);
  const quotes = items.flatMap((item) => [...item.matchAll(/"([^"]+)"/g)].map((m) => m[1]!.replace(/\s+/g, " ")));
  expect(quotes.length, "the notes the book quotes are found").toBeGreaterThanOrEqual(6);
  const unknown = quotes.filter((quote) => !formats.some((format) => format.startsWith(shape(quote.split("…")[0]!.trim()))));
  expect(unknown, "notes the book quotes that no format string starts with").toEqual([]);
});
